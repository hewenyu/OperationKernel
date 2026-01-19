//! Common test utilities and fixtures for tool testing
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Test fixture for file operations
pub struct TestFixture {
    /// Temporary directory that gets cleaned up automatically
    pub temp_dir: TempDir,
}

impl TestFixture {
    /// Create a new test fixture with a temporary directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        Self { temp_dir }
    }

    /// Get the path to the temporary directory
    pub fn path(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }

    /// Create a test file with given content
    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let filepath = self.path().join(name);
        std::fs::write(&filepath, content).expect("Failed to write test file");
        filepath
    }

    /// Create a subdirectory
    pub fn create_dir(&self, name: &str) -> PathBuf {
        let dirpath = self.path().join(name);
        std::fs::create_dir_all(&dirpath).expect("Failed to create test dir");
        dirpath
    }

    /// Create a binary file (for testing binary detection)
    pub fn create_binary_file(&self, name: &str) -> PathBuf {
        let filepath = self.path().join(name);
        let binary_data: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
        std::fs::write(&filepath, binary_data).expect("Failed to write binary file");
        filepath
    }

    /// Read file content
    pub fn read_file(&self, name: &str) -> String {
        let filepath = self.path().join(name);
        std::fs::read_to_string(&filepath).expect("Failed to read test file")
    }

    /// Check if file exists
    pub fn file_exists(&self, name: &str) -> bool {
        self.path().join(name).exists()
    }

    /// Create multiple files at once (directory tree)
    /// Example: vec![("src/main.rs", "code"), ("README.md", "docs")]
    pub fn create_tree(&self, files: Vec<(&str, &str)>) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for (path, content) in files {
            // Create parent directories if needed
            let filepath = self.path().join(path);
            if let Some(parent) = filepath.parent() {
                std::fs::create_dir_all(parent).expect("Failed to create parent dirs");
            }
            std::fs::write(&filepath, content).expect("Failed to write file");
            paths.push(filepath);
        }
        paths
    }

    /// Create a .gitignore file with given patterns
    pub fn create_gitignore(&self, patterns: &[&str]) {
        let content = patterns.join("\n");
        self.create_file(".gitignore", &content);
    }

    /// Create nested directories
    /// Example: &["src", "src/tool", "tests"]
    pub fn create_nested_dirs(&self, paths: &[&str]) -> Vec<PathBuf> {
        paths
            .iter()
            .map(|p| {
                let dirpath = self.path().join(p);
                std::fs::create_dir_all(&dirpath).expect("Failed to create nested dir");
                dirpath
            })
            .collect()
    }

    /// Initialize a git repository in the temp directory (useful for .gitignore tests).
    pub fn git_init(&self) {
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(self.path())
            .status()
            .expect("Failed to run git init");
        assert!(status.success(), "git init failed");

        // Make the repo usable if a future test needs commits.
        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(self.path())
            .status();
        let _ = Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(self.path())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_creation() {
        let fixture = TestFixture::new();
        assert!(fixture.path().exists());
    }

    #[test]
    fn test_create_and_read_file() {
        let fixture = TestFixture::new();
        fixture.create_file("test.txt", "hello world");
        assert_eq!(fixture.read_file("test.txt"), "hello world");
    }

    #[test]
    fn test_create_dir() {
        let fixture = TestFixture::new();
        let dir = fixture.create_dir("subdir");
        assert!(dir.exists());
        assert!(dir.is_dir());
    }
}
