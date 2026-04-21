use std::process::{Command, Output};

use tracing::{info, warn};

use crate::admin::is_admin;
use crate::config::launcher::{Firewall, Network};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub enum Role {
    Game,
    A2s,
    Rcon,
}

impl Role {
    pub fn suffix(self) -> &'static str {
        match self {
            Role::Game => "GAME",
            Role::A2s => "A2S",
            Role::Rcon => "RCON",
        }
    }
    pub fn port(self, net: &Network) -> u16 {
        match self {
            Role::Game => net.game_port,
            Role::A2s => net.a2s_port,
            Role::Rcon => net.rcon_port,
        }
    }
    pub const ALL: [Role; 3] = [Role::Game, Role::A2s, Role::Rcon];
}

pub fn rule_name(prefix: &str, role: Role) -> String {
    format!("{prefix} {}", role.suffix())
}

/// Idempotent: delete all three rules first, then add with current ports.
pub fn add(fw: &Firewall, net: &Network) -> Result<()> {
    if !is_admin() {
        return Err(Error::NeedsAdmin);
    }
    for role in Role::ALL {
        let name = rule_name(&fw.rule_prefix, role);
        let _ = netsh_delete(&name);
    }
    for role in Role::ALL {
        let name = rule_name(&fw.rule_prefix, role);
        netsh_add(&name, role.port(net))?;
        info!(rule = %name, port = role.port(net), "firewall rule added");
    }
    Ok(())
}

/// Delete the three rules; treat "no match" as success.
pub fn remove(fw: &Firewall) -> Result<()> {
    if !is_admin() {
        return Err(Error::NeedsAdmin);
    }
    for role in Role::ALL {
        let name = rule_name(&fw.rule_prefix, role);
        let _ = netsh_delete(&name);
        info!(rule = %name, "firewall rule removed (or was absent)");
    }
    Ok(())
}

/// Log-only check at supervisor startup. Inspects current rules and warns if
/// prefix rules exist but ports don't match launcher.toml.
pub fn startup_check(fw: &Firewall, net: &Network) {
    let existing: Vec<(Role, Option<u16>)> = Role::ALL
        .iter()
        .map(|&role| {
            let name = rule_name(&fw.rule_prefix, role);
            (role, netsh_show(&name).unwrap_or(None))
        })
        .collect();

    let any_exist = existing.iter().any(|(_, p)| p.is_some());
    if !any_exist {
        info!(
            "Firewall rules not managed by launcher. \
             Run `armafield-server firewall add` if external clients need access."
        );
        return;
    }

    let mismatch = existing
        .iter()
        .any(|(role, found)| found.is_none_or(|p| p != role.port(net)));
    if mismatch {
        warn!(
            existing = ?existing,
            expected_game = net.game_port,
            expected_a2s = net.a2s_port,
            expected_rcon = net.rcon_port,
            "firewall rules exist with mismatched ports; run `armafield-server firewall add` to sync"
        );
    } else {
        tracing::debug!("firewall rules OK");
    }
}

fn netsh_add(name: &str, port: u16) -> Result<()> {
    let out = run_netsh(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        &format!("name={name}"),
        "dir=in",
        "action=allow",
        "protocol=UDP",
        &format!("localport={port}"),
    ])?;
    if !out.status.success() {
        return Err(Error::Firewall(format!(
            "add rule '{name}' failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

fn netsh_delete(name: &str) -> Result<()> {
    let _ = run_netsh(&[
        "advfirewall",
        "firewall",
        "delete",
        "rule",
        &format!("name={name}"),
    ])?;
    Ok(())
}

fn netsh_show(name: &str) -> Result<Option<u16>> {
    let out = run_netsh(&[
        "advfirewall",
        "firewall",
        "show",
        "rule",
        &format!("name={name}"),
    ])?;
    if !out.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    if text.contains("No rules match") {
        return Ok(None);
    }
    for line in text.lines() {
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("localport:") {
            if let Ok(p) = rest.trim().parse::<u16>() {
                return Ok(Some(p));
            }
        }
    }
    // Rule exists but we couldn't parse its port; signal "present, unknown" to the caller.
    Ok(Some(0))
}

fn run_netsh(args: &[&str]) -> std::io::Result<Output> {
    Command::new("netsh").args(args).output()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_suffix_is_stable() {
        assert_eq!(Role::Game.suffix(), "GAME");
        assert_eq!(Role::A2s.suffix(), "A2S");
        assert_eq!(Role::Rcon.suffix(), "RCON");
    }

    #[test]
    fn rule_name_concatenates_prefix_and_suffix() {
        assert_eq!(rule_name("ArmaField", Role::Game), "ArmaField GAME");
        assert_eq!(rule_name("MyServer", Role::Rcon), "MyServer RCON");
    }

    #[test]
    fn role_port_reads_from_network() {
        let net = Network {
            public_address: String::new(),
            game_port: 5,
            a2s_port: 6,
            rcon_port: 7,
        };
        assert_eq!(Role::Game.port(&net), 5);
        assert_eq!(Role::A2s.port(&net), 6);
        assert_eq!(Role::Rcon.port(&net), 7);
    }
}