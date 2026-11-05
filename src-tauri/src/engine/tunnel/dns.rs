//! Answers DNS queries that arrive through the TUN interface from the same DNS
//! over HTTPS resolver the proxy engine uses, so DNS is filtered the same way
//! no matter which resolver the system is configured with.

use std::net::IpAddr;

use smoltcp::phy::ChecksumCapabilities;
use smoltcp::wire::{
    IpAddress, IpProtocol, Ipv4Address, Ipv4Packet, Ipv4Repr, Ipv6Address, Ipv6Packet, Ipv6Repr,
    UdpPacket, UdpRepr,
};

use crate::engine::proxy::Resolver;

const HEADER: usize = 12;
const TYPE_A: u16 = 1;
const TYPE_AAAA: u16 = 28;
const CLASS_IN: u16 = 1;
const MAX_ANSWERS: usize = 16;
const TTL: u32 = 60;

struct Question {
    id: u16,
    name: String,
    kind: u16,
    class: u16,
    end: usize,
}

/// Build a DNS response for a raw query, or `None` when the query cannot be
/// parsed. Queries for records other than A are answered empty; that makes
/// browsers fall back to IPv4 while every name still goes through DoH.
pub async fn respond(resolver: &Resolver, query: &[u8]) -> Option<Vec<u8>> {
    let question = parse(query)?;
    if question.kind == TYPE_A && question.class == CLASS_IN {
        match resolver.lookup(&question.name).await {
            Ok(addresses) => {
                let addresses: Vec<IpAddr> =
                    addresses.into_iter().filter(|address| address.is_ipv4()).collect();
                Some(answer(query, &question, &addresses, 0))
            }
            Err(_) => Some(answer(query, &question, &[], 3)),
        }
    } else if question.kind == TYPE_AAAA {
        Some(answer(query, &question, &[], 0))
    } else {
        Some(answer(query, &question, &[], 0))
    }
}

fn answer(query: &[u8], question: &Question, addresses: &[IpAddr], rcode: u8) -> Vec<u8> {
    let addresses = &addresses[..addresses.len().min(MAX_ANSWERS)];
    let mut response = Vec::with_capacity(HEADER + question.end - HEADER + addresses.len() * 16);
    response.extend_from_slice(&question.id.to_be_bytes());
    response.extend_from_slice(&0x8180u16.to_be_bytes());
    response[3] |= rcode & 0x0f;
    response.extend_from_slice(&1u16.to_be_bytes());
    response.extend_from_slice(&(addresses.len() as u16).to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&query[HEADER..question.end]);
    for address in addresses {
        response.extend_from_slice(&[0xc0, 0x0c]);
        response.extend_from_slice(&TYPE_A.to_be_bytes());
        response.extend_from_slice(&CLASS_IN.to_be_bytes());
        response.extend_from_slice(&TTL.to_be_bytes());
        response.extend_from_slice(&4u16.to_be_bytes());
        if let IpAddr::V4(address) = address {
            response.extend_from_slice(&address.octets());
        }
    }
    response
}

fn parse(query: &[u8]) -> Option<Question> {
    if query.len() < HEADER + 5 {
        return None;
    }
    let id = u16::from_be_bytes([query[0], query[1]]);
    if query[2] & 0x80 != 0 {
        return None;
    }
    let questions = u16::from_be_bytes([query[4], query[5]]);
    if questions != 1 {
        return None;
    }
    let mut at = HEADER;
    let mut labels = Vec::new();
    loop {
        let length = *query.get(at)? as usize;
        at += 1;
        if length == 0 {
            break;
        }
        if length & 0xc0 != 0 || length > 63 {
            return None;
        }
        let label = query.get(at..at + length)?;
        labels.push(String::from_utf8_lossy(label).to_ascii_lowercase());
        at += length;
        if labels.len() > 32 {
            return None;
        }
    }
    let kind = u16::from_be_bytes([*query.get(at)?, *query.get(at + 1)?]);
    let class = u16::from_be_bytes([*query.get(at + 2)?, *query.get(at + 3)?]);
    let end = at + 4;
    if end > query.len() {
        return None;
    }
    Some(Question { id, name: labels.join("."), kind, class, end })
}

/// Wrap a DNS message in an IPv4 or IPv6 packet with UDP headers.
pub fn datagram(src: IpAddr, dst: IpAddr, src_port: u16, dst_port: u16, payload: &[u8]) -> Option<Vec<u8>> {
    let checksums = ChecksumCapabilities::default();
    let udp = UdpRepr { src_port, dst_port };

    match (src, dst) {
        (IpAddr::V4(src), IpAddr::V4(dst)) => {
            let repr = Ipv4Repr {
                src_addr: Ipv4Address::from(src),
                dst_addr: Ipv4Address::from(dst),
                next_header: IpProtocol::Udp,
                payload_len: 8 + payload.len(),
                hop_limit: 64,
            };
            let mut buffer = vec![0u8; repr.buffer_len() + 8 + payload.len()];
            let mut packet = Ipv4Packet::new_unchecked(&mut buffer);
            repr.emit(&mut packet, &checksums);
            let mut udp_packet = UdpPacket::new_unchecked(packet.payload_mut());
            udp.emit(
                &mut udp_packet,
                &IpAddress::Ipv4(repr.src_addr),
                &IpAddress::Ipv4(repr.dst_addr),
                payload.len(),
                |data| data.copy_from_slice(payload),
                &checksums,
            );
            Some(buffer)
        }
        (IpAddr::V6(src), IpAddr::V6(dst)) => {
            let repr = Ipv6Repr {
                src_addr: Ipv6Address::from(src),
                dst_addr: Ipv6Address::from(dst),
                next_header: IpProtocol::Udp,
                payload_len: 8 + payload.len(),
                hop_limit: 64,
            };
            let mut buffer = vec![0u8; repr.buffer_len() + 8 + payload.len()];
            let mut packet = Ipv6Packet::new_unchecked(&mut buffer);
            repr.emit(&mut packet);
            let mut udp_packet = UdpPacket::new_unchecked(packet.payload_mut());
            udp.emit(
                &mut udp_packet,
                &IpAddress::Ipv6(repr.src_addr),
                &IpAddress::Ipv6(repr.dst_addr),
                payload.len(),
                |data| data.copy_from_slice(payload),
                &checksums,
            );
            Some(buffer)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(name: &str, kind: u16) -> Vec<u8> {
        let mut buffer = vec![0u8; HEADER];
        buffer[0..2].copy_from_slice(&0x1234u16.to_be_bytes());
        buffer[2..4].copy_from_slice(&0x0100u16.to_be_bytes());
        buffer[4..6].copy_from_slice(&1u16.to_be_bytes());
        for label in name.split('.') {
            buffer.push(label.len() as u8);
            buffer.extend_from_slice(label.as_bytes());
        }
        buffer.push(0);
        buffer.extend_from_slice(&kind.to_be_bytes());
        buffer.extend_from_slice(&CLASS_IN.to_be_bytes());
        buffer
    }

    #[test]
    fn parses_questions() {
        let parsed = parse(&query("Discord.com", TYPE_A)).unwrap();
        assert_eq!(parsed.name, "discord.com");
        assert_eq!(parsed.kind, TYPE_A);
        assert_eq!(parsed.id, 0x1234);
    }

    #[test]
    fn builds_a_response_with_answers() {
        let question = parse(&query("example.com", TYPE_A)).unwrap();
        let addresses = ["93.184.216.34".parse::<IpAddr>().unwrap()];
        let response = answer(&query("example.com", TYPE_A), &question, &addresses, 0);
        assert_eq!(&response[0..2], &0x1234u16.to_be_bytes());
        assert_eq!(response[3] & 0x0f, 0);
        assert_eq!(u16::from_be_bytes([response[6], response[7]]), 1);
        assert!(response.ends_with(&[93, 184, 216, 34]));
    }

    #[test]
    fn ipv4_datagram_has_checksums() {
        let packet = datagram(
            "1.2.3.4".parse().unwrap(),
            "8.8.8.8".parse().unwrap(),
            53,
            4444,
            b"hello",
        )
        .unwrap();
        assert_eq!(packet[0], 0x45);
        assert_eq!(packet[9], PROTOCOL_UDP);
        assert_ne!(&packet[10..12], &[0, 0]);
        assert_eq!(&packet[12..16], &[1, 2, 3, 4]);
        assert_eq!(packet.len(), 28 + 5);
    }

    const PROTOCOL_UDP: u8 = 17;
}
