use assert_cmd::Command;
use predicates::str::contains;

fn bin() -> Command {
    Command::cargo_bin("armafield-server").expect("binary built")
}

#[test]
fn top_level_help() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Arma Reforger"));
}

#[test]
fn run_help() {
    bin()
        .args(["run", "--help"])
        .assert()
        .success()
        .stdout(contains("supervisor"));
}

#[test]
fn firewall_help() {
    bin().args(["firewall", "--help"]).assert().success();
    bin().args(["firewall", "add", "--help"]).assert().success();
    bin()
        .args(["firewall", "remove", "--help"])
        .assert()
        .success();
}

#[test]
fn service_help_hides_run() {
    let out = bin().args(["service", "--help"]).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(!text.contains("_run"), "service _run must be hidden");
}

#[test]
fn config_help() {
    bin().args(["config", "--help"]).assert().success();
    bin().args(["config", "check", "--help"]).assert().success();
}
