use std::io::{self, IsTerminal};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::paths::Paths;

/// Returned guard must be held alive by `main` for the entire process lifetime;
/// dropping it flushes and shuts down the non-blocking writer thread.
pub struct LogGuards {
    _file: WorkerGuard,
}

/// Initialise tracing. `console` toggles the stderr layer - pass `false` in
/// service mode where there is no attached console.
pub fn init_tracing(paths: &Paths, console: bool) -> io::Result<LogGuards> {
    std::fs::create_dir_all(&paths.logs_dir)?;

    let file_appender = rolling::daily(&paths.logs_dir, "launcher.log");
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("armafield_server=info"));

    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .json();

    if console {
        let console_layer = fmt::layer()
            .with_writer(io::stderr)
            .with_ansi(io::stderr().is_terminal())
            .with_target(true);
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(console_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .init();
    }

    Ok(LogGuards { _file: file_guard })
}
