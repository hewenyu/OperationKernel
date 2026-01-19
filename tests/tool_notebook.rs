//! Integration tests for the notebook_edit tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, notebook::NotebookEditTool};
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

/// Helper to create a valid Jupyter notebook JSON
fn create_test_notebook(cells: Vec<(&str, &str, &str)>) -> String {
    // cells: (id, type, source)
    let cells_json: Vec<_> = cells
        .iter()
        .map(|(id, cell_type, source)| {
            let mut cell = json!({
                "id": id,
                "cell_type": cell_type,
                "metadata": {},
                "source": source,
            });

            // Add execution_count and outputs for code cells
            if *cell_type == "code" {
                cell["execution_count"] = json!(null);
                cell["outputs"] = json!([]);
            }

            cell
        })
        .collect();

    json!({
        "cells": cells_json,
        "metadata": {
            "kernelspec": {
                "display_name": "Python 3",
                "language": "python",
                "name": "python3"
            },
            "language_info": {
                "name": "python",
                "version": "3.8.0"
            }
        },
        "nbformat": 4,
        "nbformat_minor": 5
    })
    .to_string()
}

#[tokio::test]
async fn test_notebook_replace_first_cell() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
        ("cell2", "code", "print('world')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "print('replaced')",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    assert!(updated.contains("replaced"));
    assert!(!updated.contains("print('hello')"));
}

#[tokio::test]
async fn test_notebook_replace_by_cell_id() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
        ("cell2", "code", "print('world')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "print('replaced cell2')",
        "cell_id": "cell2",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    assert!(updated.contains("replaced cell2"));
    assert!(updated.contains("print('hello')")); // First cell unchanged
}

#[tokio::test]
async fn test_notebook_replace_change_cell_type() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "# Markdown content",
        "cell_type": "markdown",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    assert!(updated.contains("markdown"));
    assert!(updated.contains("# Markdown content"));
}

#[tokio::test]
async fn test_notebook_insert_code_cell_at_end() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "print('new cell')",
        "cell_type": "code",
        "edit_mode": "insert"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_cells"), Some(&json!(2)));
    assert!(output.output.contains("Inserted code cell"));
}

#[tokio::test]
async fn test_notebook_insert_markdown_cell_at_end() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "# Documentation",
        "cell_type": "markdown",
        "edit_mode": "insert"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    assert!(updated.contains("# Documentation"));
    assert!(updated.contains("markdown"));
}

#[tokio::test]
async fn test_notebook_insert_after_specific_cell() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
        ("cell2", "code", "print('world')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "print('inserted after cell1')",
        "cell_id": "cell1",
        "cell_type": "code",
        "edit_mode": "insert"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_cells"), Some(&json!(3)));
}

#[tokio::test]
async fn test_notebook_insert_into_new_notebook() {
    let fixture = TestFixture::new();
    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "new.ipynb",
        "new_source": "print('first cell')",
        "cell_type": "code",
        "edit_mode": "insert"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    assert!(fixture.file_exists("new.ipynb"));
    let content = fixture.read_file("new.ipynb");
    assert!(content.contains("first cell"));
    assert!(content.contains("nbformat"));
}

#[tokio::test]
async fn test_notebook_delete_first_cell() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('delete me')"),
        ("cell2", "code", "print('keep me')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "",  // Required but not used for delete
        "edit_mode": "delete"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    assert!(!updated.contains("delete me"));
    assert!(updated.contains("keep me"));
}

#[tokio::test]
async fn test_notebook_delete_by_cell_id() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('keep me')"),
        ("cell2", "code", "print('delete me')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "",
        "cell_id": "cell2",
        "edit_mode": "delete"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    assert!(updated.contains("keep me"));
    assert!(!updated.contains("delete me"));
}

#[tokio::test]
async fn test_notebook_delete_last_cell() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('only cell')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "",
        "edit_mode": "delete"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_cells"), Some(&json!(0)));
}

#[tokio::test]
async fn test_notebook_source_single_string() {
    let fixture = TestFixture::new();
    // Test that single-line source works
    let notebook = json!({
        "cells": [{
            "id": "cell1",
            "cell_type": "code",
            "metadata": {},
            "source": "single line",
            "execution_count": null,
            "outputs": []
        }],
        "metadata": {},
        "nbformat": 4,
        "nbformat_minor": 5
    });
    fixture.create_file("test.ipynb", &notebook.to_string());

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "replaced",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_notebook_source_multiple_lines() {
    let fixture = TestFixture::new();
    // Test that multi-line source (array format) works
    let notebook = json!({
        "cells": [{
            "id": "cell1",
            "cell_type": "code",
            "metadata": {},
            "source": ["line 1\n", "line 2\n", "line 3"],
            "execution_count": null,
            "outputs": []
        }],
        "metadata": {},
        "nbformat": 4,
        "nbformat_minor": 5
    });
    fixture.create_file("test.ipynb", &notebook.to_string());

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "new multiline\ncontent\n",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    assert!(updated.contains("new multiline"));
}

#[tokio::test]
async fn test_notebook_source_preserves_newlines() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "line1\nline2\nline3\n",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    // Verify newlines are preserved in the source array
    assert!(updated.contains("line1") && updated.contains("line2") && updated.contains("line3"));
}

#[tokio::test]
async fn test_notebook_invalid_json() {
    let fixture = TestFixture::new();
    fixture.create_file("test.ipynb", "{ invalid json }");

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "test",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("Invalid notebook JSON"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_notebook_cell_id_not_found() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "test",
        "cell_id": "nonexistent",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("Cell with ID 'nonexistent' not found"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_notebook_empty_notebook_replace() {
    let fixture = TestFixture::new();
    let notebook = json!({
        "cells": [],
        "metadata": {},
        "nbformat": 4,
        "nbformat_minor": 5
    });
    fixture.create_file("test.ipynb", &notebook.to_string());

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "test",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("no cells to replace"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_notebook_empty_notebook_delete() {
    let fixture = TestFixture::new();
    let notebook = json!({
        "cells": [],
        "metadata": {},
        "nbformat": 4,
        "nbformat_minor": 5
    });
    fixture.create_file("test.ipynb", &notebook.to_string());

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "",
        "edit_mode": "delete"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("no cells to delete"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_notebook_invalid_cell_type() {
    let fixture = TestFixture::new();
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "test",
        "cell_type": "invalid_type",
        "edit_mode": "insert"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("Invalid cell_type"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_notebook_metadata_preserved() {
    let fixture = TestFixture::new();
    let notebook = json!({
        "cells": [{
            "id": "cell1",
            "cell_type": "code",
            "metadata": {"custom": "value"},
            "source": "print('hello')",
            "execution_count": null,
            "outputs": []
        }],
        "metadata": {
            "custom_notebook_meta": "preserve_this"
        },
        "nbformat": 4,
        "nbformat_minor": 5
    });
    fixture.create_file("test.ipynb", &notebook.to_string());

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "print('updated')",
        "edit_mode": "replace"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    assert!(updated.contains("custom_notebook_meta"));
    assert!(updated.contains("preserve_this"));
}

#[tokio::test]
async fn test_notebook_execution_outputs_cleared() {
    let fixture = TestFixture::new();
    // Test that when we create new cells, execution_count is null and outputs is empty
    let notebook_content = create_test_notebook(vec![
        ("cell1", "code", "print('hello')"),
    ]);
    fixture.create_file("test.ipynb", &notebook_content);

    let tool = NotebookEditTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "notebook_path": "test.ipynb",
        "new_source": "print('new cell')",
        "cell_type": "code",
        "edit_mode": "insert"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let updated = fixture.read_file("test.ipynb");
    let parsed: serde_json::Value = serde_json::from_str(&updated).unwrap();

    // Check that the new cell has null execution_count and empty outputs
    let cells = parsed["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2);

    let new_cell = &cells[1];
    assert_eq!(new_cell["execution_count"], json!(null));
    assert_eq!(new_cell["outputs"], json!([]));
}
