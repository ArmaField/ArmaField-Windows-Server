use std::fs;
use std::path::Path;

use serde_json::Value;
use tracing::{info, warn};

use crate::config::arma::ArmaConfig;

/// Override `cfg.game.scenarioId` from MapSeeding.json. Never fails - invalid
/// or missing input results in a log line and the original scenarioId is kept.
pub fn apply_map_seeding(cfg: &mut ArmaConfig, seeding: &Path) {
    if !seeding.exists() {
        info!(path = %seeding.display(), "MapSeeding.json not found, using scenarioId from config.json");
        return;
    }

    let raw = match fs::read_to_string(seeding) {
        Ok(s) => s,
        Err(e) => {
            warn!(path = %seeding.display(), error = %e, "failed to read MapSeeding.json");
            return;
        }
    };

    let data: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            warn!(path = %seeding.display(), error = %e, "MapSeeding.json is not valid JSON");
            return;
        }
    };

    let mission = data
        .get("MissionResourceName")
        .and_then(Value::as_str)
        .map(str::trim);
    let mission = match mission {
        Some(m) if !m.is_empty() => m,
        _ => {
            warn!(path = %seeding.display(), "MissionResourceName missing or empty");
            return;
        }
    };

    if !mission.contains('{') || !mission.ends_with(".conf") {
        warn!(
            path = %seeding.display(),
            mission = mission,
            "MissionResourceName does not look like a scenario resource - ignoring"
        );
        return;
    }

    if let Some(game) = cfg
        .as_value_mut()
        .get_mut("game")
        .and_then(Value::as_object_mut)
    {
        game.insert("scenarioId".to_string(), Value::String(mission.to_string()));
        info!(
            mission = mission,
            "loaded next mission from MapSeeding.json"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::arma::ArmaConfig;
    use tempfile::tempdir;

    fn cfg_with_fallback() -> ArmaConfig {
        ArmaConfig(serde_json::json!({
            "game": { "scenarioId": "{ABC}Missions/Fallback.conf" }
        }))
    }

    fn scenario_of(cfg: &ArmaConfig) -> String {
        cfg.as_value()["game"]["scenarioId"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn missing_file_keeps_fallback() {
        let dir = tempdir().unwrap();
        let mut cfg = cfg_with_fallback();
        apply_map_seeding(&mut cfg, &dir.path().join("mapseeding.json"));
        assert_eq!(scenario_of(&cfg), "{ABC}Missions/Fallback.conf");
    }

    #[test]
    fn valid_override_applies() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("mapseeding.json");
        std::fs::write(
            &p,
            r#"{"SeedingLevel": 2, "MissionResourceName": "{XYZ}Missions/Next.conf"}"#,
        )
        .unwrap();
        let mut cfg = cfg_with_fallback();
        apply_map_seeding(&mut cfg, &p);
        assert_eq!(scenario_of(&cfg), "{XYZ}Missions/Next.conf");
    }

    #[test]
    fn invalid_json_keeps_fallback() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("mapseeding.json");
        std::fs::write(&p, "{corrupt").unwrap();
        let mut cfg = cfg_with_fallback();
        apply_map_seeding(&mut cfg, &p);
        assert_eq!(scenario_of(&cfg), "{ABC}Missions/Fallback.conf");
    }

    #[test]
    fn missing_key_keeps_fallback() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("mapseeding.json");
        std::fs::write(&p, r#"{"SeedingLevel": 1}"#).unwrap();
        let mut cfg = cfg_with_fallback();
        apply_map_seeding(&mut cfg, &p);
        assert_eq!(scenario_of(&cfg), "{ABC}Missions/Fallback.conf");
    }

    #[test]
    fn empty_value_keeps_fallback() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("mapseeding.json");
        std::fs::write(&p, r#"{"SeedingLevel": 1, "MissionResourceName": "   "}"#).unwrap();
        let mut cfg = cfg_with_fallback();
        apply_map_seeding(&mut cfg, &p);
        assert_eq!(scenario_of(&cfg), "{ABC}Missions/Fallback.conf");
    }

    #[test]
    fn suspicious_value_keeps_fallback() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("mapseeding.json");
        std::fs::write(
            &p,
            r#"{"SeedingLevel": 1, "MissionResourceName": "totally-not-a-mission"}"#,
        )
        .unwrap();
        let mut cfg = cfg_with_fallback();
        apply_map_seeding(&mut cfg, &p);
        assert_eq!(scenario_of(&cfg), "{ABC}Missions/Fallback.conf");
    }

    #[test]
    fn config_without_game_section_is_tolerated() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("mapseeding.json");
        std::fs::write(
            &p,
            r#"{"SeedingLevel": 1, "MissionResourceName": "{A}M.conf"}"#,
        )
        .unwrap();
        let mut cfg = ArmaConfig(serde_json::json!({"bindPort": 2001}));
        apply_map_seeding(&mut cfg, &p);
        assert!(cfg.as_value().get("game").is_none());
    }
}
