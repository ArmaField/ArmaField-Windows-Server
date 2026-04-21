use std::path::Path;
use std::process::{Command, ExitStatus};

use tracing::info;

use crate::error::Result;

/// Abstraction over invoking the steamcmd binary. Tests swap in a mock.
pub trait SteamcmdRunner {
    fn run(&self, args: &[&str]) -> std::io::Result<ExitStatus>;
}

pub struct RealSteamcmd<'a> {
    pub steamcmd_exe: &'a Path,
}

impl<'a> SteamcmdRunner for RealSteamcmd<'a> {
    fn run(&self, args: &[&str]) -> std::io::Result<ExitStatus> {
        Command::new(self.steamcmd_exe).args(args).status()
    }
}

pub fn run_steamcmd_validate<R: SteamcmdRunner>(
    runner: &R,
    install_dir: &Path,
    app_id: &str,
) -> Result<ExitStatus> {
    let install_dir_str = install_dir.to_string_lossy().to_string();
    let args = &[
        "+force_install_dir",
        install_dir_str.as_str(),
        "+login",
        "anonymous",
        "+app_update",
        app_id,
        "validate",
        "+quit",
    ];
    info!(args = ?args, "running steamcmd");
    let status = runner.run(args)?;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::os::windows::process::ExitStatusExt;

    struct Mock {
        called: RefCell<Vec<Vec<String>>>,
        exit: i32,
    }

    impl SteamcmdRunner for Mock {
        fn run(&self, args: &[&str]) -> std::io::Result<ExitStatus> {
            self.called
                .borrow_mut()
                .push(args.iter().map(|s| s.to_string()).collect());
            Ok(ExitStatus::from_raw(self.exit as u32))
        }
    }

    #[test]
    fn success_passes_expected_args() {
        let mock = Mock {
            called: RefCell::new(Vec::new()),
            exit: 0,
        };
        let status = run_steamcmd_validate(&mock, Path::new("C:\\af\\server"), "1874900").unwrap();
        assert!(status.success());

        let calls = mock.called.borrow();
        assert_eq!(calls.len(), 1);
        let args = &calls[0];
        assert!(args.iter().any(|a| a == "+force_install_dir"));
        assert!(args.iter().any(|a| a == "C:\\af\\server"));
        assert!(args.iter().any(|a| a == "+login"));
        assert!(args.iter().any(|a| a == "anonymous"));
        assert!(args.iter().any(|a| a == "+app_update"));
        assert!(args.iter().any(|a| a == "1874900"));
        assert!(args.iter().any(|a| a == "validate"));
        assert!(args.iter().any(|a| a == "+quit"));
    }

    #[test]
    fn failure_is_propagated_as_non_zero_exit() {
        let mock = Mock {
            called: RefCell::new(Vec::new()),
            exit: 1,
        };
        let status = run_steamcmd_validate(&mock, Path::new("C:\\af\\server"), "1874900").unwrap();
        assert!(!status.success());
    }
}