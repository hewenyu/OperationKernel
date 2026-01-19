//! Integration tests for the grep tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, grep::GrepTool};
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
async fn test_grep_basic_match() {
    let fixture = TestFixture::new();
    fixture.create_tree(vec![
        ("file1.txt", "Hello World\nFoo Bar\nHello Again"),
        ("file2.txt", "No match here\nJust text"),
    ]);

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "Hello"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("file1.txt"));
    assert!(output.output.contains("Hello World"));
    assert!(output.output.contains("Hello Again"));
    assert!(!output.output.contains("file2.txt"));
    assert_eq!(output.metadata.get("total_matches"), Some(&json!(2)));
}

#[tokio::test]
async fn test_grep_no_match() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "Nothing special here");

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "NOTFOUND"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("No matches found"));
    assert_eq!(output.metadata.get("total_matches"), Some(&json!(0)));
}

#[tokio::test]
async fn test_grep_case_insensitive() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "Hello WORLD\nhello world\nHELLO");

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "hello",
        "case_sensitive": false
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_matches"), Some(&json!(3)));
}

#[tokio::test]
async fn test_grep_context_lines() {
    let fixture = TestFixture::new();
    fixture.create_file(
        "test.txt",
        "Line 1\nLine 2\nMATCH HERE\nLine 4\nLine 5"
    );

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH",
        "context_lines": 1
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should contain match line and 1 line before/after
    assert!(output.output.contains("Line 2"));
    assert!(output.output.contains("MATCH HERE"));
    assert!(output.output.contains("Line 4"));
    // Should NOT contain lines outside context
    assert!(!output.output.contains("Line 1"));
    assert!(!output.output.contains("Line 5"));
}

#[tokio::test]
async fn test_grep_max_results() {
    let fixture = TestFixture::new();
    // Create many matching lines
    let mut content = String::new();
    for i in 1..=150 {
        content.push_str(&format!("MATCH line {}\n", i));
    }
    fixture.create_file("test.txt", &content);

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH",
        "max_results": 50
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("shown_matches"), Some(&json!(50)));
    assert!(output.output.contains("Showing 50 of"));
}

#[tokio::test]
async fn test_grep_invalid_regex() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "content");

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "[invalid("  // Invalid regex
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("Invalid regex"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_grep_respects_gitignore() {
    let fixture = TestFixture::new();
    fixture.git_init();

    // Create .gitignore first
    fixture.create_gitignore(&["ignored.txt"]);

    // Then create the files
    fixture.create_tree(vec![
        ("tracked.txt", "MATCH in tracked"),
        ("ignored.txt", "MATCH in ignored"),
    ]);

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("tracked.txt"));
    assert!(!output.output.contains("ignored.txt"));
    assert_eq!(output.metadata.get("total_matches"), Some(&json!(1)));
}

#[tokio::test]
async fn test_grep_include_patterns() {
    let fixture = TestFixture::new();
    fixture.create_tree(vec![
        ("file.rs", "MATCH in rust"),
        ("file.txt", "MATCH in text"),
        ("file.md", "MATCH in markdown"),
    ]);

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH",
        "include_patterns": ["*.rs", "*.md"]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("file.rs"));
    assert!(output.output.contains("file.md"));
    assert!(!output.output.contains("file.txt"));
}

#[tokio::test]
async fn test_grep_exclude_patterns() {
    let fixture = TestFixture::new();
    fixture.create_tree(vec![
        ("file.rs", "MATCH in rust"),
        ("file.txt", "MATCH in text"),
    ]);

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH",
        "exclude_patterns": ["*.txt"]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("file.rs"));
    assert!(!output.output.contains("file.txt"));
}

#[tokio::test]
async fn test_grep_include_and_exclude_patterns_combined() {
    let fixture = TestFixture::new();
    fixture.create_tree(vec![
        ("keep.rs", "MATCH in rust"),
        ("drop.txt", "MATCH in text"),
        ("also.rs", "MATCH in rust too"),
    ]);

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH",
        "include_patterns": ["*.rs", "*.txt"],
        "exclude_patterns": ["*.txt"]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("keep.rs"));
    assert!(output.output.contains("also.rs"));
    assert!(!output.output.contains("drop.txt"));
}

#[tokio::test]
async fn test_grep_skips_binary_files() {
    let fixture = TestFixture::new();
    fixture.create_file("text.txt", "MATCH in text");
    fixture.create_binary_file("binary.bin");

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("text.txt"));
    assert!(
        output.metadata.get("binary_files_skipped").is_some()
        || output.metadata.get("binary_files_skipped") == Some(&json!(1))
    );
}

#[tokio::test]
async fn test_grep_nested_directories() {
    let fixture = TestFixture::new();
    fixture.create_tree(vec![
        ("root.txt", "MATCH at root"),
        ("dir1/file1.txt", "MATCH in dir1"),
        ("dir1/dir2/file2.txt", "MATCH in dir2"),
    ]);

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_matches"), Some(&json!(3)));
    assert!(output.output.contains("root.txt"));
    assert!(output.output.contains("dir1/file1.txt") || output.output.contains("dir1"));
    assert!(output.output.contains("dir2/file2.txt") || output.output.contains("dir2"));
}

#[tokio::test]
async fn test_grep_search_specific_path() {
    let fixture = TestFixture::new();
    fixture.create_tree(vec![
        ("dir1/file.txt", "MATCH in dir1"),
        ("dir2/file.txt", "MATCH in dir2"),
    ]);

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "MATCH",
        "path": "dir1"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_matches"), Some(&json!(1)));
    assert!(output.output.contains("dir1"));
    assert!(!output.output.contains("dir2"));
}

#[tokio::test]
async fn test_grep_file_not_found() {
    let fixture = TestFixture::new();

    let tool = GrepTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "pattern": "test",
        "path": "nonexistent"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::FileNotFound(_) => {},
        _ => panic!("Expected FileNotFound error"),
    }
}
