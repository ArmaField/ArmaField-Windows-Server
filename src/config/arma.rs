use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::error::{Error, Result};

/// Thin wrapper around a serde_json::Value representing the parsed
/// Arma Reforger server config. We keep it open-ended so new fields
/// in config.json don't break parsing.
#[derive(Debug, Clone)]
pub struct ArmaConfig(pub Value);

impl ArmaConfig {
    pub fn as_value(&self) -> &Value {
        &self.0
    }
    pub fn as_value_mut(&mut self) -> &mut Value {
        &mut self.0
    }
}

/// Read config.json, validate the minimum contract: it is a JSON object,
/// contains `game` (object), and `game.scenarioId` (non-empty string).
pub fn load_user_config(path: &Path) -> Result<ArmaConfig> {
    if !path.exists() {
        return Err(Error::ConfigNotFound(path.to_path_buf()));
    }
    let raw = fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&raw)?;

    let game = value
        .get("game")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            Error::Config(format!(
                "{}: missing or non-object 'game' section",
                path.display()
            ))
        })?;

    let scenario = game
        .get("scenarioId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if scenario.is_none() {
        return Err(Error::Config(format!(
            "{}: missing or empty 'game.scenarioId'",
            path.display()
        )));
    }

    Ok(ArmaConfig(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(path: &Path, content: &str) {
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn missing_file_returns_config_not_found() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("nope.json");
        match load_user_config(&p) {
            Err(Error::ConfigNotFound(got)) => assert_eq!(got, p),
            other => panic!("expected ConfigNotFound, got {other:?}"),
        }
    }

    #[test]
    fn invalid_json_returns_json_error() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("bad.json");
        write(&p, "{not valid json");
        let err = load_user_config(&p).unwrap_err();
        assert!(matches!(err, Error::Json(_)));
    }

    #[test]
    fn missing_game_section_is_config_error() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("c.json");
        write(&p, r#"{"bindPort": 2001}"#);
        let err = load_user_config(&p).unwrap_err();
        match err {
            Error::Config(msg) => assert!(msg.contains("game")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn missing_scenario_id_is_config_error() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("c.json");
        write(&p, r#"{"game": {"name": "test"}}"#);
        let err = load_user_config(&p).unwrap_err();
        match err {
            Error::Config(msg) => assert!(msg.contains("scenarioId")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn empty_scenario_id_is_config_error() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("c.json");
        write(&p, r#"{"game": {"scenarioId": "   "}}"#);
        let err = load_user_config(&p).unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }

    #[test]
    fn valid_config_loads() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("c.json");
        write(&p, r#"{"game": {"scenarioId": "{A}M.conf", "name": "t"}}"#);
        let cfg = load_user_config(&p).unwrap();
        let scenario = cfg.as_value()["game"]["scenarioId"].as_str().unwrap();
        assert_eq!(scenario, "{A}M.conf");
    }
}
