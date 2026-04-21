use std::process::ExitCode as StdExitCode;
use std::sync::mpsc;

use anyhow::Context;
use clap::Parser;
use tracing::{error, info, warn};

use armafield_server::cli::{Cli, Command, ConfigAction, FirewallAction, ServiceAction};
use armafield_server::config::launcher::Launcher;
use armafield_server::error::{ExitCode, Result};
use armafield_server::logging::init_tracing;
use armafield_server::paths::Paths;
use armafield_server::{admin, config::arma, firewall, service, steamcmd, supervisor};

const MAX_PATH_WARN_THRESHOLD: usize = 200;

fn main() -> StdExitCode {
    set_utf8_console();

    let cli = Cli::parse();
    let code = match dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            // Domain errors carry their own exit code; anyhow would otherwise
            // collapse everything to IoFatal and break the documented contract.
            match e.downcast_ref::<armafield_server::error::Error>() {
                Some(inner) => inner.exit_code(),
                None => ExitCode::IoFatal,
            }
        }
    };
    StdExitCode::from(code.as_i32() as u8)
}

fn set_utf8_console() {
    use windows::Win32::System::Console::{SetConsoleCP, SetConsoleOutputCP};
    const CP_UTF8: u32 = 65001;
    unsafe {
        let _ = SetConsoleOutputCP(CP_UTF8);
        let _ = SetConsoleCP(CP_UTF8);
    }
}

fn dispatch(cli: Cli) -> anyhow::Result<ExitCode> {
    // `service _run` runs under SCM with its own logging; bypass the
    // console-oriented init below.
    if let Command::Service {
        action: ServiceAction::Run,
    } = &cli.command
    {
        let paths = Paths::resolve()?;
        let cfg = Launcher::load(&paths.launcher_toml)?;
        service::run::dispatch(&cfg.service.name).context("service dispatcher failed")?;
        return Ok(ExitCode::Ok);
    }

    let paths = Paths::resolve()?;
    let _guards = init_tracing(&paths, true)?;

    if paths.home.as_os_str().len() > MAX_PATH_WARN_THRESHOLD {
        warn!(
            home = %paths.home.display(),
            len = paths.home.as_os_str().len(),
            "installation path is long - some Arma Reforger paths may exceed Windows MAX_PATH (260)"
        );
    }

    match cli.command {
        Command::Run => cmd_run(&paths),
        Command::Install => cmd_install(&paths),
        Command::Update => cmd_update(&paths),
        Command::Config {
            action: ConfigAction::Check,
        } => cmd_config_check(&paths),
        Command::Firewall {
            action: FirewallAction::Add,
        } => cmd_firewall(&paths, true),
        Command::Firewall {
            action: FirewallAction::Remove,
        } => cmd_firewall(&paths, false),
        Command::Service { action } => cmd_service(action, &paths),
    }
}

fn cmd_run(paths: &Paths) -> anyhow::Result<ExitCode> {
    let cfg = Launcher::load(&paths.launcher_toml)?;
    let (tx, rx) = mpsc::channel::<()>();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .context("failed to install Ctrl+C handler")?;
    let code = supervisor::run_supervisor(&cfg, paths, rx, false);
    Ok(code)
}

fn cmd_install(paths: &Paths) -> anyhow::Result<ExitCode> {
    let cfg = Launcher::load(&paths.launcher_toml)?;

    if !paths.launcher_toml.exists() {
        let src = std::env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot find launcher.example.toml next to exe"))?
            .join("launcher.example.toml");
        if src.exists() {
            std::fs::copy(&src, &paths.launcher_toml)?;
            info!(from = %src.display(), to = %paths.launcher_toml.display(), "seeded launcher.toml");
        } else {
            warn!("launcher.example.toml not bundled next to exe - skipping seed");
        }
    }
    if !paths.config_json.exists() {
        let src = std::env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot find example_config.json next to exe"))?
            .join("example_config.json");
        if src.exists() {
            std::fs::copy(&src, &paths.config_json)?;
            info!(from = %src.display(), to = %paths.config_json.display(), "seeded config.json");
        } else {
            warn!("example_config.json not bundled next to exe - skipping seed");
        }
    }

    steamcmd::install::ensure_steamcmd(&paths.steamcmd_dir, &paths.steamcmd_exe)?;

    let runner = steamcmd::validate::RealSteamcmd {
        steamcmd_exe: &paths.steamcmd_exe,
    };
    let status = steamcmd::validate::run_steamcmd_validate(
        &runner,
        &paths.server_dir,
        &cfg.steamcmd.app_id,
    )?;
    if !status.success() {
        error!(status = ?status, "steamcmd validate failed");
        return Ok(ExitCode::SteamcmdFatal);
    }
    std::fs::create_dir_all(&paths.state_dir)?;
    std::fs::File::create(&paths.marker)?;
    info!("install complete");
    Ok(ExitCode::Ok)
}

fn cmd_update(paths: &Paths) -> anyhow::Result<ExitCode> {
    let cfg = Launcher::load(&paths.launcher_toml)?;
    steamcmd::install::ensure_steamcmd(&paths.steamcmd_dir, &paths.steamcmd_exe)?;
    let runner = steamcmd::validate::RealSteamcmd {
        steamcmd_exe: &paths.steamcmd_exe,
    };
    let status = steamcmd::validate::run_steamcmd_validate(
        &runner,
        &paths.server_dir,
        &cfg.steamcmd.app_id,
    )?;
    if !status.success() {
        return Ok(ExitCode::SteamcmdFatal);
    }
    std::fs::create_dir_all(&paths.state_dir)?;
    std::fs::File::create(&paths.marker)?;
    Ok(ExitCode::Ok)
}

fn cmd_config_check(paths: &Paths) -> anyhow::Result<ExitCode> {
    let _ = Launcher::load(&paths.launcher_toml)?;
    let _ = arma::load_user_config(&paths.config_json)?;
    info!("configuration OK");
    Ok(ExitCode::Ok)
}

fn cmd_firewall(paths: &Paths, add: bool) -> anyhow::Result<ExitCode> {
    if !admin::is_admin() {
        error!("this command requires administrator privileges");
        return Ok(ExitCode::WindowsAccess);
    }
    let cfg = Launcher::load(&paths.launcher_toml)?;
    let res: Result<()> = if add {
        firewall::add(&cfg.firewall, &cfg.network)
    } else {
        firewall::remove(&cfg.firewall)
    };
    match res {
        Ok(()) => Ok(ExitCode::Ok),
        Err(e) => {
            error!(error = %e, "firewall operation failed");
            Ok(e.exit_code())
        }
    }
}

fn cmd_service(action: ServiceAction, paths: &Paths) -> anyhow::Result<ExitCode> {
    let cfg = Launcher::load(&paths.launcher_toml)?;
    let svc = &cfg.service;

    let res: Result<ExitCode> = match action {
        ServiceAction::Install {
            auto,
            manual,
            disabled,
        } => {
            let start = if auto {
                service::manage::StartTypeOverride::Auto
            } else if manual {
                service::manage::StartTypeOverride::Manual
            } else if disabled {
                service::manage::StartTypeOverride::Disabled
            } else {
                service::manage::StartTypeOverride::FromConfig
            };
            service::manage::install(svc, start, paths.home.clone()).map(|()| ExitCode::Ok)
        }
        ServiceAction::Uninstall => service::manage::uninstall(svc).map(|()| ExitCode::Ok),
        ServiceAction::Start => service::manage::start(svc).map(|()| ExitCode::Ok),
        ServiceAction::Stop => service::manage::stop(svc).map(|()| ExitCode::Ok),
        ServiceAction::Status => {
            let s = service::manage::status(svc)?;
            println!("{s}");
            Ok(ExitCode::Ok)
        }
        ServiceAction::Run => unreachable!("handled at dispatch()"),
    };
    match res {
        Ok(code) => Ok(code),
        Err(e) => {
            error!(error = %e, "service operation failed");
            Ok(e.exit_code())
        }
    }
}