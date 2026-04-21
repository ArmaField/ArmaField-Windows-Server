use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Network {
    #[serde(default)]
    pub public_address: String,
    #[serde(default = "default_game_port")]
    pub game_port: u16,
    #[serde(default = "default_a2s_port")]
    pub a2s_port: u16,
    #[serde(default = "default_rcon_port")]
    pub rcon_port: u16,
}

fn default_game_port() -> u16 {
    2001
}
fn default_a2s_port() -> u16 {
    17777
}
fn default_rcon_port() -> u16 {
    19999
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Arma {
    #[serde(default = "default_params")]
    pub params: String,
}

fn default_params() -> String {
    "-maxFPS 120 -backendlog -nothrow".to_string()
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SteamCmd {
    #[serde(default = "default_check_interval")]
    pub check_interval_minutes: u64,
    #[serde(default)]
    pub skip_install: bool,
    #[serde(default = "default_app_id")]
    pub app_id: String,
}

fn default_check_interval() -> u64 {
    60
}
fn default_app_id() -> String {
    "1874900".to_string()
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Firewall {
    #[serde(default = "default_rule_prefix")]
    pub rule_prefix: String,
    #[serde(default)]
    pub auto_manage: bool,
}

fn default_rule_prefix() -> String {
    "ArmaField".to_string()
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Service {
    #[serde(default = "default_service_name")]
    pub name: String,
    #[serde(default = "default_service_display_name")]
    pub display_name: String,
    #[serde(default = "default_service_description")]
    pub description: String,
    #[serde(default = "default_start_type")]
    pub start_type: String,
}

fn default_service_name() -> String {
    "ArmaFieldServer".to_string()
}
fn default_service_display_name() -> String {
    "ArmaField Reforger Server".to_string()
}
fn default_service_description() -> String {
    "Runs the Arma Reforger dedicated server with ArmaField MapSeeding rotation and auto-updates."
        .to_string()
}
fn default_start_type() -> String {
    "auto".to_string()
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Launcher {
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub arma: Arma,
    #[serde(default)]
    pub steamcmd: SteamCmd,
    #[serde(default)]
    pub firewall: Firewall,
    #[serde(default)]
    pub service: Service,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            public_address: String::new(),
            game_port: default_game_port(),
            a2s_port: default_a2s_port(),
            rcon_port: default_rcon_port(),
        }
    }
}

impl Default for Arma {
    fn default() -> Self {
        Self {
            params: default_params(),
        }
    }
}

impl Default for SteamCmd {
    fn default() -> Self {
        Self {
            check_interval_minutes: default_check_interval(),
            skip_install: false,
            app_id: default_app_id(),
        }
    }
}

impl Default for Firewall {
    fn default() -> Self {
        Self {
            rule_prefix: default_rule_prefix(),
            auto_manage: false,
        }
    }
}

impl Default for Service {
    fn default() -> Self {
        Self {
            name: default_service_name(),
            display_name: default_service_display_name(),
            description: default_service_description(),
            start_type: default_start_type(),
        }
    }
}

impl Launcher {
    /// Read and parse `launcher.toml`. If the file is missing, return a
    /// fully-defaulted struct - the launcher can still run on defaults.
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(s) => Ok(toml::from_str::<Launcher>(&s)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Launcher {
                network: Network::default(),
                arma: Arma::default(),
                steamcmd: SteamCmd::default(),
                firewall: Firewall::default(),
                service: Service::default(),
            }),
            Err(e) => Err(Error::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_full_example() {
        let toml_str = r#"
            [network]
            public_address = "203.0.113.42"
            game_port      = 2001
            a2s_port       = 17777
            rcon_port      = 19999

            [arma]
            params = "-maxFPS 144"

            [steamcmd]
            check_interval_minutes = 30
            skip_install           = true
            app_id                 = "1874900"

            [firewall]
            rule_prefix = "AF"
            auto_manage = true

            [service]
            name         = "SvcName"
            display_name = "Svc Display"
            description  = "Svc Desc"
            start_type   = "manual"
        "#;
        let l: Launcher = toml::from_str(toml_str).unwrap();
        assert_eq!(l.network.public_address, "203.0.113.42");
        assert_eq!(l.network.game_port, 2001);
        assert_eq!(l.arma.params, "-maxFPS 144");
        assert_eq!(l.steamcmd.check_interval_minutes, 30);
        assert!(l.steamcmd.skip_install);
        assert_eq!(l.firewall.rule_prefix, "AF");
        assert!(l.firewall.auto_manage);
        assert_eq!(l.service.name, "SvcName");
        assert_eq!(l.service.start_type, "manual");
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("does-not-exist.toml");
        let l = Launcher::load(&p).unwrap();
        assert_eq!(l.network.game_port, 2001);
        assert_eq!(l.steamcmd.app_id, "1874900");
        assert!(!l.firewall.auto_manage);
        assert_eq!(l.service.start_type, "auto");
    }

    #[test]
    fn empty_sections_use_defaults() {
        let l: Launcher = toml::from_str("[network]\n").unwrap();
        assert_eq!(l.network.game_port, 2001);
        assert_eq!(l.network.public_address, "");
        assert_eq!(l.arma.params, "-maxFPS 120 -backendlog -nothrow");
    }

    #[test]
    fn unknown_field_rejected() {
        let toml_str = r#"
            [network]
            game_port = 2001
            bogus     = 42
        "#;
        let err = toml::from_str::<Launcher>(toml_str).unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn invalid_port_rejected() {
        let toml_str = r#"
            [network]
            game_port = "not a number"
        "#;
        let err = toml::from_str::<Launcher>(toml_str).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("game_port"));
    }
}
