//! Integration tests for the edit tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, edit::EditTool};
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
async fn test_edit_simple_replacement() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "Hello World\nGoodbye World\n");

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "old_string": "Hello World",
        "new_string": "Hi Earth"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Successfully edited"));
    assert!(output.output.contains("Hi Earth"));

    // Verify file content
    let content = fixture.read_file("test.txt");
    assert!(content.contains("Hi Earth"));
    assert!(!content.contains("Hello World"));
    assert!(content.contains("Goodbye World")); // Other content unchanged
}

#[tokio::test]
async fn test_edit_replace_all() {
    let fixture = TestFixture::new();
    fixture.create_file(
        "test.txt",
        "foo bar foo\nfoo baz\nqux foo\n",
    );

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "old_string": "foo",
        "new_string": "replaced",
        "replace_all": true
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(
        output.metadata.get("replacements").unwrap(),
        &json!(4),
        "Should replace all 4 occurrences"
    );

    // Verify all instances replaced
    let content = fixture.read_file("test.txt");
    assert_eq!(content.matches("replaced").count(), 4);
    assert_eq!(content.matches("foo").count(), 0);
}

#[tokio::test]
async fn test_edit_multiple_matches_error() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "test test test\n");

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "old_string": "test",
        "new_string": "pass",
        "replace_all": false
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    // Verify error message mentions multiple matches
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Multiple matches") || err_msg.contains("3 occurrences"));
}

#[tokio::test]
async fn test_edit_string_not_found() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "Hello World\n");

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "old_string": "Nonexistent String",
        "new_string": "Something"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not found") || err_msg.contains("Nonexistent"));
}

#[tokio::test]
async fn test_edit_file_not_found() {
    let fixture = TestFixture::new();

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "nonexistent.txt",
        "old_string": "old",
        "new_string": "new"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not found") || err_msg.contains("nonexistent"));
}

#[tokio::test]
async fn test_edit_identical_strings() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "Hello World\n");

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "old_string": "Hello",
        "new_string": "Hello"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("must be different") || err_msg.contains("identical"));
}

#[tokio::test]
async fn test_edit_preserves_indentation() {
    let fixture = TestFixture::new();
    fixture.create_file(
        "code.rs",
        "fn main() {\n    println!(\"old\");\n}\n",
    );

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "code.rs",
        "old_string": "    println!(\"old\");",
        "new_string": "    println!(\"new\");"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let content = fixture.read_file("code.rs");
    assert!(content.contains("    println!(\"new\");"));
    // Verify indentation is preserved (4 spaces)
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines[1].starts_with("    "));
}

#[tokio::test]
async fn test_edit_multiline_replacement() {
    let fixture = TestFixture::new();
    fixture.create_file(
        "test.txt",
        "Line 1\nOld Block\nLine 2\nLine 3\n",
    );

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "old_string": "Old Block",
        "new_string": "New\nMultiline\nBlock"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let content = fixture.read_file("test.txt");
    assert!(content.contains("New\nMultiline\nBlock"));
    assert!(!content.contains("Old Block"));
}

#[tokio::test]
async fn test_edit_with_special_characters() {
    let fixture = TestFixture::new();
    fixture.create_file(
        "code.rs",
        r#"let x = "hello \"world\"";"#,
    );

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "code.rs",
        "old_string": r#"let x = "hello \"world\"";"#,
        "new_string": r#"let x = "goodbye \"universe\"";"#
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let content = fixture.read_file("code.rs");
    assert!(content.contains(r#"let x = "goodbye \"universe\"";"#));
}

#[tokio::test]
async fn test_edit_absolute_path() {
    let fixture = TestFixture::new();
    let file_path = fixture.create_file("test.txt", "Old Content\n");

    let tool = EditTool;
    // Use empty working dir - absolute path should work regardless
    let ctx = create_test_context(std::path::PathBuf::from("/tmp"));

    let params = json!({
        "file_path": file_path,
        "old_string": "Old Content",
        "new_string": "New Content"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("New Content"));
}

#[tokio::test]
async fn test_edit_binary_file_error() {
    let fixture = TestFixture::new();
    fixture.create_binary_file("binary.dat");

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "binary.dat",
        "old_string": "test",
        "new_string": "pass"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    eprintln!("Actual error message: {}", err_msg);
    assert!(
        err_msg.to_lowercase().contains("binary") || err_msg.contains("utf"),
        "Expected error message to mention binary file, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_edit_shows_diff() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "Line 1\nLine 2\nLine 3\n");

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "old_string": "Line 2",
        "new_string": "Modified Line 2"
    });

    let result = tool.execute(params, &ctx).await.unwrap();

    // Check that output contains diff markers
    assert!(result.output.contains("---") || result.output.contains("+++"));
    assert!(result.output.contains("-") || result.output.contains("+"));
}

#[tokio::test]
async fn test_edit_metadata() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "foo bar foo\n");

    let tool = EditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "file_path": "test.txt",
        "old_string": "foo",
        "new_string": "baz",
        "replace_all": true
    });

    let result = tool.execute(params, &ctx).await.unwrap();

    // Verify metadata
    assert!(result.metadata.contains_key("filepath"));
    assert!(result.metadata.contains_key("replacements"));
    assert!(result.metadata.contains_key("replace_all"));

    assert_eq!(result.metadata.get("replacements").unwrap(), &json!(2));
    assert_eq!(result.metadata.get("replace_all").unwrap(), &json!(true));
}
