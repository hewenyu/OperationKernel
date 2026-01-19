use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Status of a background shell
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellStatus {
    Running,
    Completed { exit_code: Option<i32> },
    Failed { error: String },
}

/// A background shell process with captured output
pub struct BackgroundShell {
    pub id: String,
    process: Option<Child>,
    stdout_lines: Arc<Mutex<VecDeque<String>>>,
    stderr_lines: Arc<Mutex<VecDeque<String>>>,
    status: Arc<Mutex<ShellStatus>>,
    started_at: SystemTime,
    max_buffer_lines: usize,
}

impl BackgroundShell {
    /// Spawn a new background shell command
    pub async fn spawn(
        id: String,
        command: String,
        working_dir: std::path::PathBuf,
    ) -> anyhow::Result<Self> {
        tracing::debug!(
            id = %id,
            command = %command,
            working_dir = %working_dir.display(),
            "spawning background shell"
        );

        // Spawn the command with pipes
        let mut child = Command::new("bash")
            .arg("-c")
            .arg(&command)
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;

        // Take stdout/stderr handles
        let stdout = child.stdout.take().expect("stdout not captured");
        let stderr = child.stderr.take().expect("stderr not captured");

        // Create shared buffers
        let stdout_lines = Arc::new(Mutex::new(VecDeque::new()));
        let stderr_lines = Arc::new(Mutex::new(VecDeque::new()));
        let status = Arc::new(Mutex::new(ShellStatus::Running));

        // Spawn stdout reader task
        let stdout_buf = stdout_lines.clone();
        let max_lines = 10000; // Keep last 10k lines
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut buf = stdout_buf.lock().await;
                buf.push_back(line);
                if buf.len() > max_lines {
                    buf.pop_front();
                }
            }
        });

        // Spawn stderr reader task
        let stderr_buf = stderr_lines.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut buf = stderr_buf.lock().await;
                buf.push_back(line);
                if buf.len() > max_lines {
                    buf.pop_front();
                }
            }
        });

        Ok(Self {
            id,
            process: Some(child),
            stdout_lines,
            stderr_lines,
            status,
            started_at: SystemTime::now(),
            max_buffer_lines: max_lines,
        })
    }

    /// Get the shell ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get current status
    pub async fn status(&self) -> ShellStatus {
        self.status.lock().await.clone()
    }

    /// Get stdout lines (all available)
    pub async fn stdout_lines(&self) -> Vec<String> {
        self.stdout_lines.lock().await.iter().cloned().collect()
    }

    /// Get stderr lines (all available)
    pub async fn stderr_lines(&self) -> Vec<String> {
        self.stderr_lines.lock().await.iter().cloned().collect()
    }

    /// Get new stdout lines since last offset
    pub async fn stdout_since(&self, offset: usize) -> Vec<String> {
        let lines = self.stdout_lines.lock().await;
        if offset >= lines.len() {
            vec![]
        } else {
            lines.iter().skip(offset).cloned().collect()
        }
    }

    /// Get new stderr lines since last offset
    pub async fn stderr_since(&self, offset: usize) -> Vec<String> {
        let lines = self.stderr_lines.lock().await;
        if offset >= lines.len() {
            vec![]
        } else {
            lines.iter().skip(offset).cloned().collect()
        }
    }

    /// Get total line counts
    pub async fn line_counts(&self) -> (usize, usize) {
        let stdout_count = self.stdout_lines.lock().await.len();
        let stderr_count = self.stderr_lines.lock().await.len();
        (stdout_count, stderr_count)
    }

    /// Check if process has finished
    pub async fn check_finished(&mut self) -> bool {
        if let Some(ref mut child) = self.process {
            match child.try_wait() {
                Ok(Some(exit_status)) => {
                    let exit_code = exit_status.code();
                    *self.status.lock().await = ShellStatus::Completed { exit_code };
                    self.process = None;
                    true
                }
                Ok(None) => false, // Still running
                Err(e) => {
                    *self.status.lock().await = ShellStatus::Failed {
                        error: e.to_string(),
                    };
                    self.process = None;
                    true
                }
            }
        } else {
            true // Already finished
        }
    }

    /// Kill the background process
    pub async fn kill(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut child) = self.process {
            child.kill().await?;
            *self.status.lock().await = ShellStatus::Completed {
                exit_code: Some(137), // SIGKILL
            };
            self.process = None;
        }
        Ok(())
    }

    /// Get uptime in seconds
    pub fn uptime_secs(&self) -> u64 {
        self.started_at
            .elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Get started timestamp
    pub fn started_at(&self) -> SystemTime {
        self.started_at
    }
}

impl Drop for BackgroundShell {
    fn drop(&mut self) {
        // Best effort kill on drop
        if let Some(ref mut child) = self.process {
            let _ = child.start_kill();
        }
    }
}
