use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const KEEP: usize = 300;
const MAX_FILE: u64 = 512 * 1024;

struct Log {
    path: Option<PathBuf>,
    lines: VecDeque<String>,
}

fn state() -> &'static Mutex<Log> {
    static LOG: OnceLock<Mutex<Log>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(Log { path: None, lines: VecDeque::new() }))
}

pub fn init(dir: PathBuf) {
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("donut-local-vpn.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_FILE) {
        let _ = fs::rename(&path, dir.join("donut-local-vpn.old.log"));
    }
    state().lock().unwrap_or_else(|e| e.into_inner()).path = Some(path);
}

pub fn write(message: impl AsRef<str>) {
    let line = format!("{} {}", timestamp(), message.as_ref());
    let mut log = state().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(path) = &log.path {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{line}");
        }
    }
    log.lines.push_back(line);
    while log.lines.len() > KEEP {
        log.lines.pop_front();
    }
}

pub fn recent() -> Vec<String> {
    state().lock().unwrap_or_else(|e| e.into_inner()).lines.iter().cloned().collect()
}

fn timestamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_recent_lines() {
        write("hello");
        assert!(recent().iter().any(|l| l.ends_with(" hello")));
        assert_eq!(timestamp().len(), 20);
    }
}
