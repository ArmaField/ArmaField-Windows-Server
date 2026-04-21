use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "armafield-server",
    version,
    about = "Arma Reforger dedicated server supervisor with ArmaField MapSeeding rotation"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Foreground supervisor loop. Ctrl+C for graceful shutdown.
    Run,

    /// Seed launcher.toml/config.json from templates, download SteamCMD,
    /// install the game. Does not start the server.
    Install,

    /// Force `steamcmd validate` once and exit.
    Update,

    /// Validate launcher.toml and config.json without starting anything.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage Windows Firewall UDP rules.
    Firewall {
        #[command(subcommand)]
        action: FirewallAction,
    },

    /// Manage the Windows Service.
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Parse and validate configuration files.
    Check,
}

#[derive(Debug, Subcommand)]
pub enum FirewallAction {
    /// Idempotently create the three UDP rules.
    Add,
    /// Delete the three rules.
    Remove,
}

#[derive(Debug, Subcommand)]
pub enum ServiceAction {
    /// Register as a Windows Service.
    Install {
        /// Start type override - wins over launcher.toml:service.start_type.
        #[arg(long, group = "start")]
        auto: bool,
        #[arg(long, group = "start")]
        manual: bool,
        #[arg(long, group = "start")]
        disabled: bool,
    },
    /// Remove the service.
    Uninstall,
    /// Start the service via SCM.
    Start,
    /// Stop the service via SCM.
    Stop,
    /// Print the service's current SCM state.
    Status,
    /// SCM entry point - never invoke directly.
    #[command(hide = true, name = "_run")]
    Run,
}
