use std::env;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub launcher_toml: PathBuf,
    pub config_json: PathBuf,
    pub runtime_config: PathBuf,
    pub marker: PathBuf,
    pub state_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub steamcmd_dir: PathBuf,
    pub steamcmd_exe: PathBuf,
    pub server_dir: PathBuf,
    pub server_exe: PathBuf,
    pub profile_dir: PathBuf,
    pub workshop_dir: PathBuf,
    pub map_seeding_json: PathBuf,
}

impl Paths {
    /// Resolve $ARMAFIELD_HOME (if set) or the directory of the running exe.
    pub fn resolve() -> Result<Self> {
        let home = match env::var_os("ARMAFIELD_HOME") {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf))
                .ok_or_else(|| {
                    Error::Config("cannot locate ARMAFIELD_HOME or exe directory".into())
                })?,
        };
        Ok(Self::from_home(home))
    }

    pub fn from_home(home: PathBuf) -> Self {
        let state_dir = home.join("state");
        Self {
            launcher_toml: home.join("launcher.toml"),
            config_json: home.join("config.json"),
            runtime_config: state_dir.join("runtime_config.json"),
            marker: state_dir.join("steamcmd.marker"),
            logs_dir: home.join("logs"),
            steamcmd_dir: home.join("steamcmd"),
            steamcmd_exe: home.join("steamcmd").join("steamcmd.exe"),
            server_dir: home.join("server"),
            server_exe: home.join("server").join("ArmaReforgerServer.exe"),
            profile_dir: home.join("profile"),
            workshop_dir: home.join("workshop"),
            map_seeding_json: home
                .join("profile")
                .join("profile")
                .join("ArmaField")
                .join("Systems")
                .join("MapSeeding.json"),
            state_dir,
            home,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_home_builds_expected_layout() {
        let p = Paths::from_home(PathBuf::from(r"C:\games\af"));
        assert_eq!(p.launcher_toml, PathBuf::from(r"C:\games\af\launcher.toml"));
        assert_eq!(p.config_json, PathBuf::from(r"C:\games\af\config.json"));
        assert_eq!(
            p.runtime_config,
            PathBuf::from(r"C:\games\af\state\runtime_config.json")
        );
        assert_eq!(
            p.marker,
            PathBuf::from(r"C:\games\af\state\steamcmd.marker")
        );
        assert_eq!(
            p.steamcmd_exe,
            PathBuf::from(r"C:\games\af\steamcmd\steamcmd.exe")
        );
        assert_eq!(
            p.server_exe,
            PathBuf::from(r"C:\games\af\server\ArmaReforgerServer.exe")
        );
    }

    #[test]
    fn map_seeding_uses_double_profile_subdir() {
        let p = Paths::from_home(PathBuf::from(r"C:\af"));
        assert_eq!(
            p.map_seeding_json,
            PathBuf::from(r"C:\af\profile\profile\ArmaField\Systems\MapSeeding.json")
        );
    }

    #[test]
    fn resolve_respects_armafield_home_env() {
        let dir = tempfile::tempdir().unwrap();
        let prev = env::var_os("ARMAFIELD_HOME");
        unsafe {
            env::set_var("ARMAFIELD_HOME", dir.path());
        }

        let paths = Paths::resolve().unwrap();
        assert_eq!(paths.home, dir.path());

        unsafe {
            match prev {
                Some(v) => env::set_var("ARMAFIELD_HOME", v),
                None => env::remove_var("ARMAFIELD_HOME"),
            }
        }
    }

    #[test]
    fn resolve_falls_back_to_exe_dir_when_env_empty() {
        let prev = env::var_os("ARMAFIELD_HOME");
        unsafe {
            env::remove_var("ARMAFIELD_HOME");
        }

        let paths = Paths::resolve().unwrap();
        let expected = env::current_exe().unwrap().parent().unwrap().to_path_buf();
        assert_eq!(paths.home, expected);

        if let Some(v) = prev {
            unsafe {
                env::set_var("ARMAFIELD_HOME", v);
            }
        }
    }
}