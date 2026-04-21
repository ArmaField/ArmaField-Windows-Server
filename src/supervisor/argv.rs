use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::error::{Error, Result};

/// Build the argv vector passed to ArmaReforgerServer.exe.
///
/// `arma_params` is shlex-parsed so users can write flags like
/// `-logLevel "high" -maxFPS 60` and they survive intact.
pub fn build_server_argv(
    binary: &Path,
    runtime_config: &Path,
    profile: &Path,
    workshop: &Path,
    arma_params: &str,
) -> Result<Vec<OsString>> {
    let mut out = vec![
        binary.as_os_str().to_owned(),
        OsStr::new("-config").to_owned(),
        runtime_config.as_os_str().to_owned(),
        OsStr::new("-profile").to_owned(),
        profile.as_os_str().to_owned(),
        OsStr::new("-addonDownloadDir").to_owned(),
        workshop.as_os_str().to_owned(),
        OsStr::new("-addonsDir").to_owned(),
        workshop.as_os_str().to_owned(),
    ];

    let trimmed = arma_params.trim();
    if !trimmed.is_empty() {
        let parts = shell_words::split(trimmed)
            .map_err(|e| Error::Config(format!("failed to parse arma.params: {e}")))?;
        for p in parts {
            out.push(OsString::from(p));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_argv() {
        let argv = build_server_argv(
            Path::new("./server/ArmaReforgerServer.exe"),
            Path::new("C:\\af\\state\\runtime.json"),
            Path::new("C:\\af\\profile"),
            Path::new("C:\\af\\workshop"),
            "-maxFPS 120 -backendlog -nothrow",
        )
        .unwrap();

        let strs: Vec<String> = argv
            .iter()
            .map(|o| o.to_string_lossy().to_string())
            .collect();

        assert_eq!(strs[0], "./server/ArmaReforgerServer.exe");
        assert!(strs.iter().any(|s| s == "-config"));
        assert!(strs.iter().any(|s| s.contains("runtime.json")));
        assert!(strs.iter().any(|s| s == "-profile"));
        assert!(strs.iter().any(|s| s == "-addonDownloadDir"));
        assert!(strs.iter().any(|s| s == "-addonsDir"));
        assert_eq!(strs.iter().filter(|s| s.ends_with("workshop")).count(), 2);
        assert!(strs.iter().any(|s| s == "-maxFPS"));
        assert!(strs.iter().any(|s| s == "120"));
        assert!(strs.iter().any(|s| s == "-backendlog"));
        assert!(strs.iter().any(|s| s == "-nothrow"));
    }

    #[test]
    fn empty_params_produces_no_extra_args() {
        let argv = build_server_argv(
            Path::new("srv"),
            Path::new("c.json"),
            Path::new("p"),
            Path::new("w"),
            "",
        )
        .unwrap();
        // binary + 8 flag/value pairs = 9 items.
        assert_eq!(argv.len(), 9);
    }

    #[test]
    fn quoted_params_parsed_by_shell_words() {
        let argv = build_server_argv(
            Path::new("srv"),
            Path::new("c.json"),
            Path::new("p"),
            Path::new("w"),
            r#"-logLevel "high warn" -maxFPS 60"#,
        )
        .unwrap();

        let strs: Vec<String> = argv
            .iter()
            .map(|o| o.to_string_lossy().to_string())
            .collect();
        assert!(strs.contains(&"high warn".to_string()));
        assert!(strs.contains(&"60".to_string()));
    }

    #[test]
    fn invalid_quoting_returns_config_error() {
        // unbalanced quote
        let err = build_server_argv(
            Path::new("srv"),
            Path::new("c.json"),
            Path::new("p"),
            Path::new("w"),
            r#"-foo "unterminated"#,
        )
        .unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }
}