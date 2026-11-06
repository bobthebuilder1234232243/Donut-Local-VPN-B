## Donut Local VPN {{version}}

### What's new

- **New name and identity.** Donut Local VPN, identifier `com.donutvpn.donutlocalvpn`, new icons, new
  About screen and new download links.
- **macOS and Linux now use a real local tunnel.** Traffic from every app goes through a TUN interface
  and the bypass engine instead of a system proxy. There is no proxy address to copy and no per-app
  setup.
- **DNS inside the tunnel** is answered over HTTPS on macOS and Linux, so the provider's resolver is
  not used while connected.
- **Linux needs root or `CAP_NET_ADMIN`** now. The app asks through `pkexec` or `sudo`. The old
  "Linux needs no root" claim no longer applies.
- **macOS asks for the administrator password when you connect.** The privileged helper exits and
  restores routes when you disconnect.
- **Removed** the "Open Discord" button, the manual proxy address and the system-proxy instructions.
- **Windows is unchanged**: the same WinDivert driver and GoodbyeDPI engine as before.

### Downloads

| Platform | File |
| --- | --- |
| Windows | `Donut-Local-VPN_{{version}}_x64-setup.exe` |
| macOS (universal) | `Donut-Local-VPN_{{version}}_universal.dmg` |
| Linux (AppImage) | `Donut-Local-VPN_{{version}}_amd64.AppImage` |
| Linux (Debian/Ubuntu) | `Donut-Local-VPN_{{version}}_amd64.deb` |

The builds are not code-signed. On macOS, right-click the app and choose **Open** the first time
(or run `xattr -dr com.apple.quarantine "/Applications/Donut Local VPN.app"`).

### Notes

- Updates are installed manually by running the new installer; the app does not check for updates.
- On macOS and Linux the tunnel carries TCP and DNS. Other UDP traffic (QUIC, some games and voice
  chat) and ICMP bypass the tunnel and may still be blocked.
- "Local VPN" means the interface is on your computer. There is no remote server and no added
  encryption; it does not hide your IP address.
