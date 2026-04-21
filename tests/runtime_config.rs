use std::path::{Path, PathBuf};
use std::time::Duration;

use assert_cmd::Command;
use tempfile::tempdir;

fn fake_server_exe() -> PathBuf {
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "--example", "fake_server"])
        .status()
        .expect("cargo build --example fake_server failed to run");
    assert!(status.success(), "cargo build --example fake_server failed");

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest
        .join("target")
        .join("debug")
        .join("examples")
        .join("fake_server.exe");
    assert!(
        candidate.exists(),
        "fake_server.exe not found at {}",
        candidate.display()
    );
    candidate
}

fn copy_as(src: &Path, dst: &Path) {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::copy(src, dst).expect("copy fake_server.exe");
}

#[test]
fn run_once_writes_runtime_config() {
    let fake = fake_server_exe();
    let home = tempdir().unwrap();
    let home_path = home.path();

    copy_as(
        &fake,
        &home_path.join("server").join("ArmaReforgerServer.exe"),
    );

    std::fs::write(
        home_path.join("launcher.toml"),
        r#"
[steamcmd]
skip_install = true
check_interval_minutes = 60

[network]
game_port = 2001
a2s_port  = 17777
rcon_port = 19999
"#,
    )
    .unwrap();

    std::fs::write(
        home_path.join("config.json"),
        r#"{"a2s":{"address":"1.1.1.1","port":1},"rcon":{"address":"1.1.1.1","port":1},"game":{"scenarioId":"{A}M.conf"}}"#,
    )
    .unwrap();

    // Fake server exits instantly; supervisor enters a 5 s CRASH_BACKOFF,
    // so an 8 s timeout is enough for one full loop iteration and is what
    // terminates the process (non-zero exit expected - we assert on the
    // on-disk artefact instead).
    let _ = Command::cargo_bin("armafield-server")
        .unwrap()
        .env("ARMAFIELD_HOME", home_path)
        .arg("run")
        .timeout(Duration::from_secs(8))
        .output()
        .unwrap();

    let runtime = home_path.join("state").join("runtime_config.json");
    assert!(
        runtime.exists(),
        "runtime_config.json should have been written to {}",
        runtime.display()
    );

    let body = std::fs::read_to_string(&runtime).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["bindAddress"], "0.0.0.0");
    assert_eq!(parsed["bindPort"], 2001);
    assert_eq!(parsed["publicPort"], 2001);
    assert_eq!(parsed["a2s"]["address"], "0.0.0.0");
    assert_eq!(parsed["a2s"]["port"], 17777);
    assert_eq!(parsed["rcon"]["address"], "0.0.0.0");
    assert_eq!(parsed["rcon"]["port"], 19999);
    assert_eq!(parsed["game"]["scenarioId"], "{A}M.conf");
}