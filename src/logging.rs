use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[allow(dead_code)]
pub struct LogGuard(tracing_appender::non_blocking::WorkerGuard);

/// Initialize debug logging.
///
/// When `debug` is enabled, logs are written to `~/.config/ok/ok-debug.log` by default.
/// When `debug` is disabled, this is a no-op.
pub fn init(debug: bool) -> Result<Option<LogGuard>> {
    if !debug {
        return Ok(None);
    }

    let log_path = default_log_path()?;
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create log directory: {}", parent.display())
        })?;
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("Failed to open log file: {}", log_path.display()))?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file);

    // Default: debug our crate, warn for everything else.
    let filter = EnvFilter::try_new("ok=debug,warn").unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_target(true)
        .with_writer(non_blocking)
        .try_init()
        .ok(); // If already initialized (e.g., in tests), don't crash.

    tracing::info!("debug logging enabled");
    tracing::info!(log_file = %log_path.display(), "writing logs to file");

    Ok(Some(LogGuard(guard)))
}

fn default_log_path() -> Result<PathBuf> {
    let config_path = crate::config::config_path()?;
    Ok(config_path.with_file_name("ok-debug.log"))
}

/// Best-effort redaction for common API key patterns (e.g. `sk-...`).
pub fn redact_secrets(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut last = 0usize;
    let mut i = 0usize;

    while i < input.len() {
        if input[i..].starts_with("sk-") && i + 3 < input.len() {
            let mut j = i + 3;
            while j < input.len() {
                match bytes[j] {
                    b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' => j += 1,
                    _ => break,
                }
            }

            // Require a minimum length to reduce false positives.
            if j.saturating_sub(i + 3) >= 8 {
                out.push_str(&input[last..i]);
                out.push_str("sk-***REDACTED***");
                last = j;
                i = j;
                continue;
            }
        }

        let ch = input[i..].chars().next().unwrap();
        i += ch.len_utf8();
    }

    out.push_str(&input[last..]);
    out
}
