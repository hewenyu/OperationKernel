use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;
use tracing_appender::non_blocking::NonBlocking;

#[allow(dead_code)]
pub struct LogGuard(tracing_appender::non_blocking::WorkerGuard);

/// Initialize debug logging.
///
/// When `debug` is enabled, logs are written to `~/.config/ok/ok-debug.log` by default.
/// When `debug` is disabled, this is a no-op.
pub fn init(config: &crate::config::Config) -> Result<Option<LogGuard>> {
    if !config.debug {
        return Ok(None);
    }

    let rotation = config
        .debug_log_rotation
        .unwrap_or(crate::config::station::DebugLogRotation::Session);
    let keep = config.debug_log_keep;

    let (writer, log_path_for_display, guard): (NonBlocking, PathBuf, tracing_appender::non_blocking::WorkerGuard) = match rotation {
        crate::config::station::DebugLogRotation::None => {
            let log_path = resolve_base_log_path(config.debug_log_path.as_deref())?;
            ensure_parent_dir(&log_path)?;

            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .with_context(|| format!("Failed to open log file: {}", log_path.display()))?;

            let (non_blocking, guard) = tracing_appender::non_blocking(file);
            (non_blocking, log_path, guard)
        }
        crate::config::station::DebugLogRotation::Daily => {
            let base = resolve_base_log_path(config.debug_log_path.as_deref())?;
            let (dir, base_name) = split_dir_and_name(&base)?;
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create log directory: {}", dir.display()))?;

            // Clean up before opening new writer to keep directory tidy.
            cleanup_rotated_logs(&dir, RotationKind::Daily { base_name: base_name.clone() }, keep)?;

            let appender = tracing_appender::rolling::daily(&dir, &base_name);
            let (non_blocking, guard) = tracing_appender::non_blocking(appender);
            (non_blocking, base, guard)
        }
        crate::config::station::DebugLogRotation::Session => {
            let base = resolve_base_log_path(config.debug_log_path.as_deref())?;
            let (dir, base_name) = split_dir_and_name(&base)?;
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create log directory: {}", dir.display()))?;

            cleanup_rotated_logs(
                &dir,
                RotationKind::Session {
                    base_name: base_name.clone(),
                },
                keep,
            )?;

            let session_path = build_session_log_path(&dir, &base_name);
            ensure_parent_dir(&session_path)?;

            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&session_path)
                .with_context(|| format!("Failed to open log file: {}", session_path.display()))?;

            let (non_blocking, guard) = tracing_appender::non_blocking(file);
            (non_blocking, session_path, guard)
        }
    };

    // Default: debug our crate, warn for everything else.
    let filter = EnvFilter::try_new("ok=debug,warn").unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_target(true)
        .with_writer(writer)
        .try_init()
        .ok(); // If already initialized (e.g., in tests), don't crash.

    tracing::info!("debug logging enabled");
    tracing::info!(log_file = %log_path_for_display.display(), rotation = ?rotation, "writing logs to file");

    Ok(Some(LogGuard(guard)))
}

fn default_log_path() -> Result<PathBuf> {
    let config_path = crate::config::config_path()?;
    Ok(config_path.with_file_name("ok-debug.log"))
}

fn resolve_base_log_path(config_value: Option<&str>) -> Result<PathBuf> {
    let Some(raw) = config_value else {
        return default_log_path();
    };

    let expanded = expand_tilde(raw);
    let path = PathBuf::from(expanded);

    // If it ends with a path separator, treat as directory.
    if raw.ends_with(std::path::MAIN_SEPARATOR) {
        return Ok(path.join("ok-debug.log"));
    }

    // If it exists and is a directory, treat as directory.
    if path.is_dir() {
        return Ok(path.join("ok-debug.log"));
    }

    // If it has an extension, treat as file path. Otherwise also treat as file path.
    Ok(path)
}

fn expand_tilde(raw: &str) -> String {
    if raw == "~" || raw.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            let suffix = raw.strip_prefix('~').unwrap_or("");
            return format!("{}{}", home.display(), suffix);
        }
    }
    raw.to_string()
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create log directory: {}", parent.display()))?;
    }
    Ok(())
}

fn split_dir_and_name(path: &Path) -> Result<(PathBuf, String)> {
    let dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .context("Invalid debug_log_path: not valid UTF-8")?
        .to_string();
    Ok((dir, name))
}

fn build_session_log_path(dir: &Path, base_name: &str) -> PathBuf {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let file_name = format!("{base_name}.session-{ts}");
    dir.join(file_name)
}

enum RotationKind {
    Daily { base_name: String },
    Session { base_name: String },
}

fn cleanup_rotated_logs(dir: &Path, kind: RotationKind, keep: Option<usize>) -> Result<()> {
    let keep = keep.unwrap_or(match kind {
        RotationKind::Daily { .. } => 7,
        RotationKind::Session { .. } => 20,
    });

    if keep == 0 {
        return Ok(());
    }

    let prefix = match &kind {
        // tracing_appender::rolling::daily uses: `{base_name}.{YYYY-MM-DD}`
        RotationKind::Daily { base_name } => format!("{base_name}."),
        RotationKind::Session { base_name } => format!("{base_name}.session-"),
    };

    let mut candidates: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read log directory: {}", dir.display()))?
    {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else { continue };
        if name.starts_with(&prefix) {
            candidates.push(name.to_string());
        }
    }

    candidates.sort();
    candidates.reverse(); // newest first (lexicographic works for our suffix formats)

    for (idx, name) in candidates.iter().enumerate() {
        if idx < keep {
            continue;
        }
        let path = dir.join(name);
        if let Err(e) = std::fs::remove_file(&path) {
            tracing::debug!(error = %e, file = %path.display(), "failed to remove old log file");
        }
    }

    Ok(())
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
