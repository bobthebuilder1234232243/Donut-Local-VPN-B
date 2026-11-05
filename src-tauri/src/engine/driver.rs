use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use super::EngineError;
use crate::catalog::{self, Region};
use crate::system::{hide_window, run};

const ERROR_ELEVATION_REQUIRED: i32 = 740;
const WARMUP: Duration = Duration::from_millis(1500);
const DNS: &[&str] = &[
    "--dns-addr", "77.88.8.8", "--dns-port", "1253",
    "--dnsv6-addr", "2a02:6b8::feed:0ff", "--dnsv6-port", "1253",
];

pub struct Engine {
    exe: PathBuf,
}

pub struct Running {
    child: Child,
}

impl Engine {
    pub fn new(resources: PathBuf) -> Self {
        Self { exe: resources.join("goodbyedpi").join("goodbyedpi.exe") }
    }

    pub fn profiles(region: Region) -> Vec<&'static str> {
        catalog::goodbyedpi_profiles(region)
    }

    pub fn recover() {}

    pub fn unload() {
        let _ = run("taskkill", &["/F", "/IM", "goodbyedpi.exe"]);
        for service in ["WinDivert1.4", "WinDivert"] {
            let _ = run("sc", &["stop", service]);
        }
    }

    pub async fn prepare(&self) -> Result<(), EngineError> {
        if !self.exe.is_file() {
            return Err(EngineError::MissingFiles);
        }
        let _ = tokio::task::spawn_blocking(|| {
            let _ = run("sc", &["stop", "GoodbyeDPI"]);
            let _ = run("taskkill", &["/F", "/IM", "goodbyedpi.exe"]);
        })
        .await;
        Ok(())
    }

    pub async fn start(&self, profile: &str, only: Option<&[String]>) -> Result<Running, EngineError> {
        let mut command = Command::new(&self.exe);
        command.args(profile.split_whitespace()).args(DNS);
        if let Some(domains) = only {
            let list = std::env::temp_dir().join("donut-local-vpn-blacklist.txt");
            std::fs::write(&list, domains.join("\n")).map_err(|e| EngineError::Failed(e.to_string()))?;
            command.arg("--blacklist").arg(list);
        }
        command
            .current_dir(self.exe.parent().unwrap_or(&self.exe))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide_window(&mut command);

        let child = command.spawn().map_err(|e| match e.raw_os_error() {
            Some(ERROR_ELEVATION_REQUIRED) => EngineError::NeedsAdmin,
            _ => EngineError::Failed(e.to_string()),
        })?;
        attach_to_job(&child);

        tokio::time::sleep(WARMUP).await;
        Ok(Running { child })
    }
}

impl Running {
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn probe_proxy(&self) -> Option<u16> {
        None
    }

    pub async fn activate(&mut self) -> Result<(), EngineError> {
        Ok(())
    }

    pub async fn refresh(&mut self) -> Result<(), EngineError> {
        Ok(())
    }

    pub async fn stop(self) {
        let mut child = self.child;
        let _ = child.kill();
        let _ = tokio::task::spawn_blocking(move || child.wait()).await;
    }
}

fn job() -> Option<HANDLE> {
    static JOB: OnceLock<usize> = OnceLock::new();
    let raw = *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return 0;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        job as usize
    });
    (raw != 0).then_some(raw as HANDLE)
}

fn attach_to_job(child: &Child) {
    if let Some(job) = job() {
        unsafe {
            AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE);
        }
    }
}
