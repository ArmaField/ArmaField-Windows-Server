use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tracing::info;
use windows_service::service::{
    ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus,
    ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use crate::admin::is_admin;
use crate::config::launcher::Service as ServiceCfg;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub enum StartTypeOverride {
    Auto,
    Manual,
    Disabled,
    FromConfig,
}

impl StartTypeOverride {
    pub fn resolve(self, cfg: &ServiceCfg) -> Result<ServiceStartType> {
        let raw = match self {
            StartTypeOverride::Auto => "auto",
            StartTypeOverride::Manual => "manual",
            StartTypeOverride::Disabled => "disabled",
            StartTypeOverride::FromConfig => cfg.start_type.as_str(),
        };
        Ok(match raw {
            "auto" => ServiceStartType::AutoStart,
            "manual" => ServiceStartType::OnDemand,
            "disabled" => ServiceStartType::Disabled,
            other => return Err(Error::Config(format!("invalid start_type '{other}'"))),
        })
    }
}

fn open_scm(access: ServiceManagerAccess) -> Result<ServiceManager> {
    ServiceManager::local_computer(None::<&str>, access)
        .map_err(|e| Error::Service(format!("open SCM failed: {e}")))
}

pub fn install(cfg: &ServiceCfg, start: StartTypeOverride, home: PathBuf) -> Result<()> {
    if !is_admin() {
        return Err(Error::NeedsAdmin);
    }
    let exe = std::env::current_exe().map_err(Error::Io)?;

    let scm = open_scm(ServiceManagerAccess::CREATE_SERVICE)?;

    let info = ServiceInfo {
        name: OsString::from(&cfg.name),
        display_name: OsString::from(&cfg.display_name),
        service_type: ServiceType::OWN_PROCESS,
        start_type: start.resolve(cfg)?,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![OsString::from("service"), OsString::from("_run")],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    let svc = scm
        .create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)
        .map_err(|e| Error::Service(format!("create_service failed: {e}")))?;

    svc.set_description(&cfg.description)
        .map_err(|e| Error::Service(format!("set_description failed: {e}")))?;

    // windows-service doesn't expose per-service env vars; write the
    // registry Environment MULTI_SZ directly so Paths::resolve picks up
    // ARMAFIELD_HOME instead of falling back to C:\Windows\System32.
    set_service_env(&cfg.name, &home)?;

    info!(name = %cfg.name, "service installed");
    Ok(())
}

fn set_service_env(name: &str, home: &std::path::Path) -> Result<()> {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_MULTI_SZ, RegCloseKey, RegOpenKeyExW,
        RegSetValueExW,
    };
    use windows::core::{HSTRING, PCWSTR};

    let key_path = format!(r"SYSTEM\CurrentControlSet\Services\{name}");
    let path_h = HSTRING::from(&key_path);
    let mut hkey = HKEY::default();
    let open_rc = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(path_h.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
    };
    if open_rc.is_err() {
        return Err(Error::Service(format!(
            "open service registry key failed for '{name}': {open_rc:?}"
        )));
    }

    let value = format!("ARMAFIELD_HOME={}", home.display());
    // MULTI_SZ: each entry NUL-terminated, whole block terminated by an extra NUL.
    let mut wide: Vec<u16> = value.encode_utf16().collect();
    wide.push(0);
    wide.push(0);
    let raw = unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
    let name_h = HSTRING::from("Environment");
    let rc = unsafe { RegSetValueExW(hkey, PCWSTR(name_h.as_ptr()), 0, REG_MULTI_SZ, Some(raw)) };
    unsafe {
        let _ = RegCloseKey(hkey);
    }

    if rc.is_err() {
        return Err(Error::Service(format!(
            "set service Environment value failed: {rc:?}"
        )));
    }
    Ok(())
}

pub fn uninstall(cfg: &ServiceCfg) -> Result<()> {
    if !is_admin() {
        return Err(Error::NeedsAdmin);
    }
    let scm = open_scm(ServiceManagerAccess::CONNECT)?;
    let svc = scm
        .open_service(
            &cfg.name,
            ServiceAccess::STOP | ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE,
        )
        .map_err(|e| Error::Service(format!("open_service failed: {e}")))?;
    if matches!(
        svc.query_status().map(|s| s.current_state),
        Ok(ServiceState::Running)
    ) {
        let _ = svc.stop();
        wait_for_state(&svc, ServiceState::Stopped, Duration::from_secs(60))?;
    }
    svc.delete()
        .map_err(|e| Error::Service(format!("delete failed: {e}")))?;
    info!(name = %cfg.name, "service uninstalled");
    Ok(())
}

pub fn start(cfg: &ServiceCfg) -> Result<()> {
    if !is_admin() {
        return Err(Error::NeedsAdmin);
    }
    let scm = open_scm(ServiceManagerAccess::CONNECT)?;
    let svc = scm
        .open_service(
            &cfg.name,
            ServiceAccess::START | ServiceAccess::QUERY_STATUS,
        )
        .map_err(|e| Error::Service(format!("open_service failed: {e}")))?;
    svc.start::<&OsStr>(&[])
        .map_err(|e| Error::Service(format!("start failed: {e}")))?;
    wait_for_state(&svc, ServiceState::Running, Duration::from_secs(30))?;
    info!(name = %cfg.name, "service started");
    Ok(())
}

pub fn stop(cfg: &ServiceCfg) -> Result<()> {
    if !is_admin() {
        return Err(Error::NeedsAdmin);
    }
    let scm = open_scm(ServiceManagerAccess::CONNECT)?;
    let svc = scm
        .open_service(&cfg.name, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS)
        .map_err(|e| Error::Service(format!("open_service failed: {e}")))?;
    svc.stop()
        .map_err(|e| Error::Service(format!("stop failed: {e}")))?;
    wait_for_state(&svc, ServiceState::Stopped, Duration::from_secs(60))?;
    info!(name = %cfg.name, "service stopped");
    Ok(())
}

pub fn status(cfg: &ServiceCfg) -> Result<String> {
    let scm = open_scm(ServiceManagerAccess::CONNECT)?;
    let svc = scm
        .open_service(&cfg.name, ServiceAccess::QUERY_STATUS)
        .map_err(|e| Error::Service(format!("open_service failed: {e}")))?;
    let st: ServiceStatus = svc
        .query_status()
        .map_err(|e| Error::Service(format!("query_status failed: {e}")))?;
    Ok(format!("{:?}", st.current_state))
}

fn wait_for_state(
    svc: &windows_service::service::Service,
    target: ServiceState,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let st = svc
            .query_status()
            .map_err(|e| Error::Service(format!("query_status failed: {e}")))?;
        if st.current_state == target {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(Error::Service(format!(
        "timed out waiting for state {:?}",
        target
    )))
}