//! Integration tests for the read tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, read::ReadTool};
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
async fn test_read_simple_file() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "Line 1\nLine 2\nLine 3");

    let tool = ReadTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Line 1"));
    assert!(output.output.contains("Line 2"));
    assert!(output.output.contains("Line 3"));
    // Check for line numbers
    assert!(output.output.contains("    1→"));
    assert!(output.output.contains("    2→"));
    assert!(output.output.contains("    3→"));
}

#[tokio::test]
async fn test_read_nonexistent_file() {
    let fixture = TestFixture::new();
    let tool = ReadTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "nonexistent.txt"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::FileNotFound(_) => {},
        _ => panic!("Expected FileNotFound error"),
    }
}

#[tokio::test]
async fn test_read_binary_file() {
    let fixture = TestFixture::new();
    fixture.create_binary_file("test.bin");

    let tool = ReadTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.bin"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::BinaryFile(_) => {},
        _ => panic!("Expected BinaryFile error"),
    }
}

#[tokio::test]
async fn test_read_with_offset() {
    let fixture = TestFixture::new();
    let content = (1..=10).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
    fixture.create_file("test.txt", &content);

    let tool = ReadTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "offset": 5  // Start from line 6
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Line 6"));
    // Check that Line 1 is not in the actual output (using line number format)
    assert!(output.output.contains("    6→Line 6"));
    assert!(!output.output.contains("    1→Line 1"));
}

#[tokio::test]
async fn test_read_with_limit() {
    let fixture = TestFixture::new();
    let content = (1..=100).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
    fixture.create_file("test.txt", &content);

    let tool = ReadTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "limit": 10  // Only read 10 lines
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Line 1"));
    assert!(output.output.contains("Line 10"));
    assert!(!output.output.contains("Line 11"));
    assert_eq!(output.metadata.get("lines_read"), Some(&json!(10)));
}

#[tokio::test]
async fn test_read_absolute_path() {
    let fixture = TestFixture::new();
    let filepath = fixture.create_file("test.txt", "Absolute path test");

    let tool = ReadTool::new();
    let ctx = create_test_context(std::path::PathBuf::from("/tmp")); // Different working dir

    let params = json!({
        "file_path": filepath.to_string_lossy()
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Absolute path test"));
}

#[tokio::test]
async fn test_read_empty_file() {
    let fixture = TestFixture::new();
    fixture.create_file("empty.txt", "");

    let tool = ReadTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "empty.txt"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_lines"), Some(&json!(0)));
}

// TODO: Add more edge case tests
// - Test line truncation (lines > 2000 chars)
// - Test byte limit truncation
// - Test various file encodings
// - Test with offset + limit combination
// - Test metadata correctness
