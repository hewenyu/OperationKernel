//! Common test utilities and fixtures for tool testing
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::sync::{Mutex, MutexGuard};
use tempfile::TempDir;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TestConfig {
    #[serde(default)]
    pub web_fetch: WebFetchTestConfig,
    #[serde(default)]
    pub web_search: WebSearchTestConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebFetchTestConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_http_url")]
    pub http_url: String,
    #[serde(default = "default_https_url")]
    pub https_url: String,
    #[serde(default = "default_cache_miss_url")]
    pub cache_miss_url: String,
    #[serde(default = "default_invalid_domain_url")]
    pub invalid_domain_url: String,

    #[serde(default = "default_redirect_same_host_url")]
    pub redirect_same_host_url: String,
    #[serde(default = "default_redirect_different_host_url")]
    pub redirect_different_host_url: String,
    #[serde(default = "default_large_page_url")]
    pub large_page_url: String,
    #[serde(default = "default_text_page_url")]
    pub text_page_url: String,

    #[serde(default = "default_concurrent_urls")]
    pub concurrent_urls: Vec<String>,
}

impl Default for WebFetchTestConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            http_url: default_http_url(),
            https_url: default_https_url(),
            cache_miss_url: default_cache_miss_url(),
            invalid_domain_url: default_invalid_domain_url(),
            redirect_same_host_url: default_redirect_same_host_url(),
            redirect_different_host_url: default_redirect_different_host_url(),
            large_page_url: default_large_page_url(),
            text_page_url: default_text_page_url(),
            concurrent_urls: default_concurrent_urls(),
        }
    }
}

fn default_http_url() -> String {
    "http://example.com".to_string()
}
fn default_https_url() -> String {
    "https://example.com".to_string()
}
fn default_cache_miss_url() -> String {
    "https://www.example.org".to_string()
}
fn default_invalid_domain_url() -> String {
    "https://this-domain-definitely-does-not-exist-12345.invalid".to_string()
}
fn default_redirect_same_host_url() -> String {
    "https://httpbin.org/redirect-to?url=https://httpbin.org/html".to_string()
}
fn default_redirect_different_host_url() -> String {
    "https://httpbin.org/redirect-to?url=https://example.com".to_string()
}
fn default_large_page_url() -> String {
    "https://httpbin.org/html".to_string()
}
fn default_text_page_url() -> String {
    "https://httpbin.org/robots.txt".to_string()
}
fn default_concurrent_urls() -> Vec<String> {
    vec![
        "https://example.com".to_string(),
        "https://example.org".to_string(),
        "https://example.net".to_string(),
    ]
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebSearchTestConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_search_provider")]
    pub provider: String,
    #[serde(default)]
    pub brave_api_key: String,

    #[serde(default = "default_search_query")]
    pub query: String,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub blocked_domains: Vec<String>,

    #[serde(default = "default_min_results")]
    pub min_results: usize,
}

impl Default for WebSearchTestConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_search_provider(),
            brave_api_key: String::new(),
            query: default_search_query(),
            allowed_domains: vec!["example.com".to_string()],
            blocked_domains: Vec::new(),
            min_results: default_min_results(),
        }
    }
}

fn default_search_provider() -> String {
    "brave".to_string()
}
fn default_search_query() -> String {
    // Keep this stable and likely to return example.com results.
    "site:example.com example domain".to_string()
}
fn default_min_results() -> usize {
    1
}

static TEST_CONFIG: OnceLock<TestConfig> = OnceLock::new();
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn test_config() -> &'static TestConfig {
    TEST_CONFIG.get_or_init(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config_path = manifest_dir.join("tests/config.toml");
        let example_path = manifest_dir.join("tests/config.example.toml");

        let raw = std::fs::read_to_string(&config_path).unwrap_or_else(|e| {
            panic!(
                "Missing or unreadable test config at {} ({e}).\n\
                 Create it by copying {} and filling in real values (API keys, URLs).",
                config_path.display(),
                example_path.display()
            )
        });

        toml::from_str(&raw).unwrap_or_else(|e| {
            panic!(
                "Invalid TOML in {} ({e}).\n\
                 Refer to {} for the expected schema.",
                config_path.display(),
                example_path.display()
            )
        })
    })
}

pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

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
