//! Logs go to the console and to a file, so a failed login can be looked at
//! after the fact without having launched the app from a terminal.
//!
//! Nothing logged may contain a token, a password or message content. The
//! bridge logs token *shape* only.

use std::{fs::File, path::PathBuf, sync::Mutex};

use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, prelude::*};

const FILE_NAME: &str = "oxicord.log";

pub fn log_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("oxicord")
        .join(FILE_NAME)
}

/// Starts logging. A log file that can't be created is not worth refusing
/// to start over, so the console alone is the fallback.
pub fn init() {
    let console = fmt::layer().with_writer(std::io::stderr);

    let path = log_path();
    let file = path
        .parent()
        .and_then(|dir| std::fs::create_dir_all(dir).ok())
        .and_then(|()| File::create(&path).ok())
        .map(|file| fmt::layer().with_ansi(false).with_writer(Mutex::new(file)));

    tracing_subscriber::registry()
        .with(LevelFilter::INFO)
        .with(console)
        .with(file)
        .init();

    tracing::info!(path = %path.display(), "logging started");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_a_log_file_that_receives_events() {
        init();
        tracing::info!("hello from the logging test");

        let text = std::fs::read_to_string(log_path()).unwrap();
        assert!(text.contains("logging started"));
        assert!(text.contains("hello from the logging test"));
    }
}
