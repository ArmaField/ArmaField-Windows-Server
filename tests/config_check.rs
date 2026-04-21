use assert_cmd::Command;
use tempfile::tempdir;

fn bin_in(home: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("armafield-server").unwrap();
    c.env("ARMAFIELD_HOME", home);
    c
}

#[test]
fn valid_config_exits_zero() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("launcher.toml"), "").unwrap();
    std::fs::write(
        dir.path().join("config.json"),
        r#"{"game":{"scenarioId":"{A}M.conf"}}"#,
    )
    .unwrap();

    bin_in(dir.path())
        .args(["config", "check"])
        .assert()
        .success();
}

#[test]
fn missing_config_json_fails_with_exit_1() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("launcher.toml"), "").unwrap();

    bin_in(dir.path())
        .args(["config", "check"])
        .assert()
        .code(1);
}

#[test]
fn invalid_json_fails_with_exit_1() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("launcher.toml"), "").unwrap();
    std::fs::write(dir.path().join("config.json"), "{not valid").unwrap();

    bin_in(dir.path())
        .args(["config", "check"])
        .assert()
        .code(1);
}

#[test]
fn invalid_launcher_toml_fails_with_exit_1() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("launcher.toml"),
        "[network\ngame_port = 2001",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("config.json"),
        r#"{"game":{"scenarioId":"{A}M.conf"}}"#,
    )
    .unwrap();

    bin_in(dir.path())
        .args(["config", "check"])
        .assert()
        .code(1);
}