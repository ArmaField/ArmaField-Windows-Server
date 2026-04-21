use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::config::arma::ArmaConfig;
use crate::config::launcher::Network;
use crate::error::Result;

/// Force bind addresses to 0.0.0.0 and set ports from launcher.toml.
/// publicAddress is set iff `network.public_address` is non-empty after trim.
pub fn apply_launcher_overrides(cfg: &mut ArmaConfig, network: &Network) {
    let value = cfg.as_value_mut();
    let obj = match value.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    obj.insert("bindAddress".into(), Value::String("0.0.0.0".into()));
    obj.insert("bindPort".into(), Value::from(network.game_port));
    obj.insert("publicPort".into(), Value::from(network.game_port));

    let public = network.public_address.trim();
    if !public.is_empty() {
        obj.insert("publicAddress".into(), Value::String(public.into()));
    }

    if let Some(a2s) = obj.get_mut("a2s").and_then(Value::as_object_mut) {
        a2s.insert("address".into(), Value::String("0.0.0.0".into()));
        a2s.insert("port".into(), Value::from(network.a2s_port));
    }

    if let Some(rcon) = obj.get_mut("rcon").and_then(Value::as_object_mut) {
        rcon.insert("address".into(), Value::String("0.0.0.0".into()));
        rcon.insert("port".into(), Value::from(network.rcon_port));
    }
}

/// Write runtime_config.json (pretty-printed, UTF-8). Creates parent dirs.
pub fn write_runtime_config(cfg: &ArmaConfig, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg.as_value())?;
    fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::launcher::Network;
    use tempfile::tempdir;

    fn full_cfg() -> ArmaConfig {
        ArmaConfig(serde_json::json!({
            "bindAddress": "1.2.3.4",
            "bindPort": 9999,
            "publicAddress": "5.6.7.8",
            "publicPort": 9999,
            "a2s":  { "address": "1.2.3.4", "port": 99 },
            "rcon": { "address": "1.2.3.4", "port": 88, "password": "x" },
            "game": { "scenarioId": "{A}M.conf" }
        }))
    }

    fn net(game: u16, a2s: u16, rcon: u16, pub_addr: &str) -> Network {
        Network {
            public_address: pub_addr.to_string(),
            game_port: game,
            a2s_port: a2s,
            rcon_port: rcon,
        }
    }

    #[test]
    fn forces_bind_addresses_to_zero() {
        let mut cfg = full_cfg();
        apply_launcher_overrides(&mut cfg, &net(2001, 17777, 19999, ""));
        assert_eq!(cfg.as_value()["bindAddress"], "0.0.0.0");
        assert_eq!(cfg.as_value()["a2s"]["address"], "0.0.0.0");
        assert_eq!(cfg.as_value()["rcon"]["address"], "0.0.0.0");
    }

    #[test]
    fn sets_all_three_ports() {
        let mut cfg = full_cfg();
        apply_launcher_overrides(&mut cfg, &net(2001, 17777, 19999, ""));
        assert_eq!(cfg.as_value()["bindPort"], 2001);
        assert_eq!(cfg.as_value()["publicPort"], 2001);
        assert_eq!(cfg.as_value()["a2s"]["port"], 17777);
        assert_eq!(cfg.as_value()["rcon"]["port"], 19999);
    }

    #[test]
    fn public_address_set_when_non_empty() {
        let mut cfg = full_cfg();
        apply_launcher_overrides(&mut cfg, &net(2001, 17777, 19999, "203.0.113.42"));
        assert_eq!(cfg.as_value()["publicAddress"], "203.0.113.42");
    }

    #[test]
    fn public_address_preserved_when_env_empty() {
        let mut cfg = full_cfg();
        apply_launcher_overrides(&mut cfg, &net(2001, 17777, 19999, ""));
        assert_eq!(cfg.as_value()["publicAddress"], "5.6.7.8");
    }

    #[test]
    fn public_address_preserved_when_env_whitespace() {
        let mut cfg = full_cfg();
        apply_launcher_overrides(&mut cfg, &net(2001, 17777, 19999, "   \t  "));
        assert_eq!(cfg.as_value()["publicAddress"], "5.6.7.8");
    }

    #[test]
    fn tolerates_missing_a2s_section() {
        let mut cfg = full_cfg();
        cfg.as_value_mut().as_object_mut().unwrap().remove("a2s");
        apply_launcher_overrides(&mut cfg, &net(2001, 17777, 19999, ""));
        assert!(cfg.as_value().get("a2s").is_none());
    }

    #[test]
    fn tolerates_missing_rcon_section() {
        let mut cfg = full_cfg();
        cfg.as_value_mut().as_object_mut().unwrap().remove("rcon");
        apply_launcher_overrides(&mut cfg, &net(2001, 17777, 19999, ""));
        assert!(cfg.as_value().get("rcon").is_none());
    }

    #[test]
    fn write_runtime_config_creates_parent_dirs_and_indents() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("nested").join("dir").join("runtime.json");
        let cfg = ArmaConfig(serde_json::json!({"a": 1, "b": 2}));
        write_runtime_config(&cfg, &out).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.contains('\n'));
        assert!(content.contains("  "));
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed, cfg.as_value().clone());
    }
}
