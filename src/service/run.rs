use std::ffi::OsString;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use tracing::{error, info};
use windows_service::define_windows_service;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

use crate::config::launcher::Launcher;
use crate::error::ExitCode;
use crate::logging::init_tracing;
use crate::paths::Paths;
use crate::supervisor::run_supervisor;

define_windows_service!(ffi_service_main, service_main);

/// Called by `main` when the binary is invoked as `armafield-server service _run`.
/// Blocks until SCM tells us to stop.
pub fn dispatch(service_name: &str) -> Result<(), windows_service::Error> {
    service_dispatcher::start(service_name, ffi_service_main)
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        error!(error = %e, "service main failed");
    }
}

fn run_service() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::resolve()?;
    let _guards = init_tracing(&paths, false)?;
    info!("service _run starting");

    let cfg = Launcher::load(&paths.launcher_toml)?;

    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    let event_handler = {
        let tx = shutdown_tx.clone();
        move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    let _ = tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        }
    };
    let status_handle = service_control_handler::register(&cfg.service.name, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::ZERO,
        process_id: None,
    })?;

    // Supervisor runs on a worker thread so the service_main thread stays
    // responsive for Interrogate pings from SCM.
    let worker = {
        let cfg = cfg.clone();
        let paths = paths.clone();
        thread::spawn(move || run_supervisor(&cfg, &paths, shutdown_rx, true))
    };

    let code = worker.join().unwrap_or(ExitCode::IoFatal);

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(code.as_i32() as u32),
        checkpoint: 0,
        wait_hint: Duration::ZERO,
        process_id: None,
    })?;

    info!(exit_code = code.as_i32(), "service _run finished");
    Ok(())
}
