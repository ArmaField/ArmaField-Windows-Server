pub mod argv;
pub mod process;

use std::sync::mpsc;
use std::time::Duration;

use tracing::{error, info, warn};

use crate::config::arma::load_user_config;
use crate::config::launcher::Launcher;
use crate::config::mapseeding::apply_map_seeding;
use crate::config::runtime::{apply_launcher_overrides, write_runtime_config};
use crate::error::{ExitCode, Result};
use crate::firewall;
use crate::paths::Paths;
use crate::steamcmd::install::ensure_steamcmd;
use crate::steamcmd::marker::should_validate;
use crate::steamcmd::validate::{RealSteamcmd, run_steamcmd_validate};
use crate::supervisor::process::{
    WaitOutcome, graceful_terminate, spawn_server, wait_for_exit_or_shutdown,
};

pub const CRASH_BACKOFF: Duration = Duration::from_secs(5);

pub fn run_supervisor(
    cfg: &Launcher,
    paths: &Paths,
    shutdown_rx: mpsc::Receiver<()>,
    service_mode: bool,
) -> ExitCode {
    firewall::startup_check(&cfg.firewall, &cfg.network);
    if cfg.firewall.auto_manage {
        if let Err(e) = firewall::add(&cfg.firewall, &cfg.network) {
            warn!(error = %e, "auto_manage firewall add failed - continuing without firewall rules");
        }
    }

    let result = run_supervisor_inner(cfg, paths, &shutdown_rx, service_mode);

    if cfg.firewall.auto_manage {
        if let Err(e) = firewall::remove(&cfg.firewall) {
            warn!(error = %e, "auto_manage firewall remove failed");
        }
    }

    result
}

fn run_supervisor_inner(
    cfg: &Launcher,
    paths: &Paths,
    shutdown_rx: &mpsc::Receiver<()>,
    service_mode: bool,
) -> ExitCode {
    loop {
        if matches!(shutdown_rx.try_recv(), Ok(())) {
            info!("shutdown requested before spawning; exiting");
            return ExitCode::Ok;
        }

        if let Err(code) = maybe_validate_game(cfg, paths) {
            return code;
        }

        match load_and_patch_config(cfg, paths) {
            Ok(()) => {}
            Err(e) => {
                error!(error = %e, "failed to prepare runtime config");
                return e.exit_code();
            }
        }

        let mut handle = match spawn_server(
            &paths.server_exe,
            &paths.runtime_config,
            &paths.profile_dir,
            &paths.workshop_dir,
            &paths.server_dir,
            &cfg.arma.params,
            service_mode,
        ) {
            Ok(h) => h,
            Err(e) => {
                error!(error = %e, "failed to spawn server");
                return e.exit_code();
            }
        };

        match wait_for_exit_or_shutdown(&mut handle, shutdown_rx) {
            WaitOutcome::Shutdown => {
                info!("shutdown received; terminating child");
                graceful_terminate(&mut handle, service_mode);
                return ExitCode::Ok;
            }
            WaitOutcome::Exited(status) => {
                warn!(status = ?status, "server exited; sleeping {:?} before restart", CRASH_BACKOFF);
                if interruptible_sleep(CRASH_BACKOFF, shutdown_rx) {
                    return ExitCode::Ok;
                }
            }
        }
    }
}

fn maybe_validate_game(cfg: &Launcher, paths: &Paths) -> std::result::Result<(), ExitCode> {
    let interval = Duration::from_secs(cfg.steamcmd.check_interval_minutes * 60);
    if !should_validate(
        &paths.server_exe,
        &paths.marker,
        interval,
        cfg.steamcmd.skip_install,
    ) {
        return Ok(());
    }
    if let Err(e) = ensure_steamcmd(&paths.steamcmd_dir, &paths.steamcmd_exe) {
        warn!(error = %e, "ensure_steamcmd failed");
        if !paths.server_exe.exists() {
            return Err(ExitCode::SteamcmdFatal);
        }
        return Ok(());
    }
    let runner = RealSteamcmd {
        steamcmd_exe: &paths.steamcmd_exe,
    };
    match run_steamcmd_validate(&runner, &paths.server_dir, &cfg.steamcmd.app_id) {
        Ok(status) if status.success() => {
            if let Err(e) = touch_marker(paths) {
                warn!(error = %e, "failed to touch steamcmd marker");
            }
            Ok(())
        }
        Ok(status) => {
            if paths.server_exe.exists() {
                warn!(status = ?status, "steamcmd failed; continuing with existing install");
                Ok(())
            } else {
                Err(ExitCode::SteamcmdFatal)
            }
        }
        Err(e) => {
            warn!(error = %e, "steamcmd invocation failed");
            if paths.server_exe.exists() {
                Ok(())
            } else {
                Err(ExitCode::SteamcmdFatal)
            }
        }
    }
}

fn touch_marker(paths: &Paths) -> Result<()> {
    std::fs::create_dir_all(&paths.state_dir)?;
    std::fs::File::create(&paths.marker)?;
    Ok(())
}

fn load_and_patch_config(cfg: &Launcher, paths: &Paths) -> Result<()> {
    let mut arma = load_user_config(&paths.config_json)?;
    apply_map_seeding(&mut arma, &paths.map_seeding_json);
    apply_launcher_overrides(&mut arma, &cfg.network);
    write_runtime_config(&arma, &paths.runtime_config)?;
    Ok(())
}

fn interruptible_sleep(d: Duration, shutdown_rx: &mpsc::Receiver<()>) -> bool {
    matches!(shutdown_rx.recv_timeout(d), Ok(()))
}