use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Process exit codes. Numeric values are part of the public contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Ok = 0,
    ConfigError = 1,
    SteamcmdFatal = 2,
    WindowsAccess = 3,
    IoFatal = 4,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Launcher domain errors. Every top-level Result bubbles up one of these.
#[derive(Debug, Error)]
pub enum Error {
    #[error("config error: {0}")]
    Config(String),

    #[error("config file not found at {0}")]
    ConfigNotFound(PathBuf),

    #[error("steamcmd failed: {0}")]
    Steamcmd(String),

    #[error("steamcmd failed and server binary is not installed")]
    SteamcmdFatal,

    #[error("this command requires administrator privileges")]
    NeedsAdmin,

    #[error("windows service error: {0}")]
    Service(String),

    #[error("firewall (netsh) error: {0}")]
    Firewall(String),

    #[error("i/o: {0}")]
    Io(#[from] io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
}

impl Error {
    /// Map a domain error to the exit code the binary should return.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Error::Config(_) | Error::ConfigNotFound(_) | Error::Toml(_) | Error::Json(_) => {
                ExitCode::ConfigError
            }
            Error::SteamcmdFatal => ExitCode::SteamcmdFatal,
            Error::Steamcmd(_) => ExitCode::SteamcmdFatal,
            Error::NeedsAdmin => ExitCode::WindowsAccess,
            Error::Service(_) | Error::Firewall(_) => ExitCode::WindowsAccess,
            Error::Io(_) => ExitCode::IoFatal,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_numeric_values_are_stable() {
        assert_eq!(ExitCode::Ok.as_i32(), 0);
        assert_eq!(ExitCode::ConfigError.as_i32(), 1);
        assert_eq!(ExitCode::SteamcmdFatal.as_i32(), 2);
        assert_eq!(ExitCode::WindowsAccess.as_i32(), 3);
        assert_eq!(ExitCode::IoFatal.as_i32(), 4);
    }

    #[test]
    fn config_error_maps_to_exit_code_1() {
        let err = Error::Config("oops".into());
        assert_eq!(err.exit_code(), ExitCode::ConfigError);
    }

    #[test]
    fn needs_admin_maps_to_exit_code_3() {
        assert_eq!(Error::NeedsAdmin.exit_code(), ExitCode::WindowsAccess);
    }

    #[test]
    fn io_error_maps_to_exit_code_4() {
        let err = Error::Io(io::Error::other("boom"));
        assert_eq!(err.exit_code(), ExitCode::IoFatal);
    }
}
