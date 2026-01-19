//! Integration tests for the write tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, write::WriteTool};
use serde_json::json;
use std::sync::Arc;

/// Helper to create a tool context for testing
fn create_test_context(working_dir: std::path::PathBuf) -> ToolContext {
    ToolContext::new(
        "test_session",
        "test_msg",
        "test_station",
        working_dir,
        Arc::new(ok::process::BackgroundShellManager::new()),
    )
}

#[tokio::test]
async fn test_write_new_file() {
    let fixture = TestFixture::new();
    let tool = WriteTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "new.txt",
        "content": "Hello, World!"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Successfully wrote to:"));
    assert!(output.output.contains("Creating new file:"));
    assert_eq!(output.metadata.get("existed"), Some(&json!(false)));
    assert_eq!(output.metadata.get("bytes_written"), Some(&json!(13)));

    // Verify file was actually written
    assert!(fixture.file_exists("new.txt"));
    assert_eq!(fixture.read_file("new.txt"), "Hello, World!");
}

#[tokio::test]
async fn test_write_overwrite_existing() {
    let fixture = TestFixture::new();
    fixture.create_file("existing.txt", "Old content");

    let tool = WriteTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "existing.txt",
        "content": "New content"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Successfully wrote to:"));
    assert_eq!(output.metadata.get("existed"), Some(&json!(true)));

    // Verify file was overwritten
    assert_eq!(fixture.read_file("existing.txt"), "New content");
}

#[tokio::test]
async fn test_write_shows_diff() {
    let fixture = TestFixture::new();
    fixture.create_file("diff.txt", "Line 1\nLine 2\nLine 3");

    let tool = WriteTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "diff.txt",
        "content": "Line 1\nModified Line 2\nLine 3"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("---"));  // Diff header
    assert!(output.output.contains("+++"));
    assert!(output.output.contains("-Line 2"));  // Removed line
    assert!(output.output.contains("+Modified Line 2"));  // Added line
}

#[tokio::test]
async fn test_write_creates_parent_directories() {
    let fixture = TestFixture::new();
    let tool = WriteTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "subdir/nested/file.txt",
        "content": "Nested file"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    // Verify file and directories were created
    assert!(fixture.path().join("subdir").exists());
    assert!(fixture.path().join("subdir/nested").exists());
    assert!(fixture.file_exists("subdir/nested/file.txt"));
}

#[tokio::test]
async fn test_write_absolute_path() {
    let fixture = TestFixture::new();
    let tool = WriteTool;
    let ctx = create_test_context(std::path::PathBuf::from("/tmp")); // Different working dir

    let filepath = fixture.path().join("absolute.txt");
    let params = json!({
        "file_path": filepath.to_string_lossy(),
        "content": "Absolute path test"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    // Verify file was written using absolute path
    assert!(filepath.exists());
}

#[tokio::test]
async fn test_write_empty_content() {
    let fixture = TestFixture::new();
    let tool = WriteTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "empty.txt",
        "content": ""
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    assert!(fixture.file_exists("empty.txt"));
    assert_eq!(fixture.read_file("empty.txt"), "");

    let output = result.unwrap();
    assert_eq!(output.metadata.get("bytes_written"), Some(&json!(0)));
}

#[tokio::test]
async fn test_write_multiline_content() {
    let fixture = TestFixture::new();
    let tool = WriteTool;
    let ctx = create_test_context(fixture.path());

    let content = "Line 1\nLine 2\nLine 3\n";
    let params = json!({
        "file_path": "multiline.txt",
        "content": content
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    assert_eq!(fixture.read_file("multiline.txt"), content);
}

// TODO: Add more edge case tests
// - Test with special characters in content
// - Test with very large files
// - Test concurrent writes
// - Test permission errors (if applicable)
