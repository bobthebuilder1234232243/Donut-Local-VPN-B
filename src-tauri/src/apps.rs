pub const REPOSITORY: &str = "https://github.com/bobthebuilder1234232243/Donut-Local-VPN-B";
pub fn open_repository() -> Result<(), String> {
    open_url(REPOSITORY)
}

pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    let program = "explorer.exe";
    #[cfg(target_os = "macos")]
    let program = "/usr/bin/open";
    #[cfg(not(any(windows, target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
