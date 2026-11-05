pub const TRAY_ARG: &str = "--tray";

#[cfg(windows)]
mod platform {
    use std::fs;

    use crate::system::{output_text, run};

    const TASK: &str = "Donut Local VPN";

    const TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Donut Local VPN</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
      <Delay>PT5S</Delay>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{args}</Arguments>
    </Exec>
  </Actions>
</Task>"#;

    pub fn enabled() -> bool {
        run("schtasks", &["/Query", "/TN", TASK])
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn set(enable: bool) -> Result<(), String> {
        if !enable {
            if !enabled() {
                return Ok(());
            }
            let out = run("schtasks", &["/Delete", "/TN", TASK, "/F"]).map_err(|e| e.to_string())?;
            return if out.status.success() { Ok(()) } else { Err(output_text(&out)) };
        }

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let user = format!(
            "{}\\{}",
            std::env::var("USERDOMAIN").unwrap_or_default(),
            std::env::var("USERNAME").unwrap_or_default()
        );
        let xml = TEMPLATE
            .replace("{user}", &escape(&user))
            .replace("{exe}", &escape(&exe.to_string_lossy()))
            .replace("{args}", super::TRAY_ARG);

        let path = std::env::temp_dir().join("donut-local-vpn-task.xml");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in xml.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(&path, bytes).map_err(|e| e.to_string())?;

        let result = run("schtasks", &["/Create", "/TN", TASK, "/XML", &path.to_string_lossy(), "/F"]);
        let _ = fs::remove_file(&path);
        let out = result.map_err(|e| e.to_string())?;
        if out.status.success() { Ok(()) } else { Err(output_text(&out)) }
    }

    fn escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::fs;
    use std::path::PathBuf;

    const LABEL: &str = "com.donutvpn.donutlocalvpn";

    fn plist_path() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library/LaunchAgents")
                .join(format!("{LABEL}.plist"))
        })
    }

    pub fn enabled() -> bool {
        plist_path().is_some_and(|p| p.exists())
    }

    pub fn set(enable: bool) -> Result<(), String> {
        let path = plist_path().ok_or("HOME is not set")?;
        if !enable {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>{arg}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#,
            exe = escape(&exe.to_string_lossy()),
            arg = super::TRAY_ARG,
        );
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        fs::write(&path, plist).map_err(|e| e.to_string())
    }

    fn escape(value: &str) -> String {
        value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::fs;
    use std::path::PathBuf;

    fn entry_path() -> Option<PathBuf> {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(config.join("autostart").join("donut-local-vpn.desktop"))
    }

    pub fn enabled() -> bool {
        entry_path().is_some_and(|p| p.exists())
    }

    pub fn set(enable: bool) -> Result<(), String> {
        let path = entry_path().ok_or("HOME is not set")?;
        if !enable {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        let exe = match std::env::var_os("APPIMAGE") {
            Some(image) => PathBuf::from(image),
            None => std::env::current_exe().map_err(|e| e.to_string())?,
        };
        let exe = exe.to_string_lossy().replace('"', "");
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName=Donut Local VPN\nExec=\"{exe}\" {arg}\nIcon=donut-local-vpn\nX-GNOME-Autostart-enabled=true\nTerminal=false\n",
            arg = super::TRAY_ARG,
        );
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        fs::write(&path, entry).map_err(|e| e.to_string())
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod platform {
    pub fn enabled() -> bool {
        false
    }

    pub fn set(_enable: bool) -> Result<(), String> {
        Err("unsupported platform".into())
    }
}

pub use platform::{enabled, set};
