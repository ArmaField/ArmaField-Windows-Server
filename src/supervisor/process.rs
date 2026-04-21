use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use tracing::{info, warn};
use windows::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
use windows::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;

use crate::error::{Error, Result};
use crate::supervisor::argv::build_server_argv;

const GRACEFUL_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ServerHandle {
    pub child: Child,
}

pub fn spawn_server(
    binary: &Path,
    runtime_config: &Path,
    profile: &Path,
    workshop: &Path,
    server_dir: &Path,
    arma_params: &str,
    service_mode: bool,
) -> Result<ServerHandle> {
    let argv = build_server_argv(binary, runtime_config, profile, workshop, arma_params)?;
    if argv.is_empty() {
        return Err(Error::Config("empty server argv".into()));
    }

    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    cmd.current_dir(server_dir);

    if service_mode {
        cmd.stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());
    } else {
        // Start the server in a new process group so GenerateConsoleCtrlEvent
        // with CTRL_BREAK_EVENT reaches only the child, not our own process.
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP.0);
    }

    info!(exe = %binary.display(), "spawning server");
    let child = cmd.spawn().map_err(Error::Io)?;
    Ok(ServerHandle { child })
}

pub enum WaitOutcome {
    Exited(std::process::ExitStatus),
    Shutdown,
}

pub fn wait_for_exit_or_shutdown(
    handle: &mut ServerHandle,
    shutdown_rx: &mpsc::Receiver<()>,
) -> WaitOutcome {
    // Poll try_wait every 200 ms while watching the shutdown channel.
    // We don't move the Child into a thread because that would complicate
    // the graceful-kill path; the poll cadence is fine for a supervisor.
    loop {
        match shutdown_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(()) => return WaitOutcome::Shutdown,
            Err(RecvTimeoutError::Disconnected) => {
                warn!("shutdown channel disconnected; treating as shutdown");
                return WaitOutcome::Shutdown;
            }
            Err(RecvTimeoutError::Timeout) => match handle.child.try_wait() {
                Ok(Some(status)) => return WaitOutcome::Exited(status),
                Ok(None) => continue,
                Err(e) => {
                    warn!(error = %e, "try_wait failed; treating as exit");
                    use std::os::windows::process::ExitStatusExt;
                    return WaitOutcome::Exited(std::process::ExitStatus::from_raw(1));
                }
            },
        }
    }
}

pub fn graceful_terminate(handle: &mut ServerHandle, service_mode: bool) {
    let pid = handle.child.id();
    if !service_mode {
        let ok = unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid).is_ok() };
        if ok {
            info!(pid, "sent CTRL_BREAK; waiting up to 30s");
            let deadline = std::time::Instant::now() + GRACEFUL_TIMEOUT;
            while std::time::Instant::now() < deadline {
                if matches!(handle.child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(200));
            }
            warn!(
                pid,
                "child did not exit after CTRL_BREAK, escalating to TerminateProcess"
            );
        } else {
            warn!(
                pid,
                "GenerateConsoleCtrlEvent failed; using TerminateProcess"
            );
        }
    }
    let _ = handle.child.kill();
    let _ = handle.child.wait();
}
