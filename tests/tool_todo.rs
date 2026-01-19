//! Integration tests for the todo_write tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, todo::TodoWriteTool};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// Helper to create a tool context for testing
fn create_test_context(session_id: &str, working_dir: PathBuf) -> ToolContext {
    ToolContext::new(
        session_id,
        "test_msg",
        "test_station",
        working_dir,
        Arc::new(ok::process::BackgroundShellManager::new()),
    )
}

#[tokio::test]
async fn test_todo_create_simple_list() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path.clone());
    let ctx = create_test_context("session1", fixture.path());

    let params = json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "pending",
                "active_form": "Working on Task 1"
            },
            {
                "content": "Task 2",
                "status": "completed",
                "active_form": "Working on Task 2"
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Task list updated"));
    assert_eq!(output.metadata.get("total_tasks").unwrap(), &json!(2));
    assert_eq!(output.metadata.get("pending").unwrap(), &json!(1));
    assert_eq!(output.metadata.get("completed").unwrap(), &json!(1));

    // Verify file was created
    assert!(storage_path.join("session1.json").exists());
}

#[tokio::test]
async fn test_todo_single_in_progress_task() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path);
    let ctx = create_test_context("session1", fixture.path());

    let params = json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "pending",
                "active_form": "Working on Task 1"
            },
            {
                "content": "Task 2",
                "status": "in_progress",
                "active_form": "Working on Task 2"
            },
            {
                "content": "Task 3",
                "status": "completed",
                "active_form": "Working on Task 3"
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("in_progress").unwrap(), &json!(1));
    assert!(output.output.contains("Current in-progress task"));
    assert!(output.output.contains("Working on Task 2"));
}

#[tokio::test]
async fn test_todo_multiple_in_progress_error() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path);
    let ctx = create_test_context("session1", fixture.path());

    let params = json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "in_progress",
                "active_form": "Working on Task 1"
            },
            {
                "content": "Task 2",
                "status": "in_progress",
                "active_form": "Working on Task 2"
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Only one task can be in_progress") || err_msg.contains("in_progress"));
}

#[tokio::test]
async fn test_todo_invalid_status() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path);
    let ctx = create_test_context("session1", fixture.path());

    let params = json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "invalid_status",
                "active_form": "Working on Task 1"
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Invalid") || err_msg.contains("invalid_status"));
}

#[tokio::test]
async fn test_todo_persistence() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path.clone());
    let ctx = create_test_context("session1", fixture.path());

    // Create initial todo list
    let params = json!({
        "todos": [
            {
                "content": "Persistent Task",
                "status": "pending",
                "active_form": "Working on Persistent Task"
            }
        ]
    });

    tool.execute(params, &ctx).await.unwrap();

    // Verify file exists and can be read
    let file_path = storage_path.join("session1.json");
    assert!(file_path.exists());

    let file_content = std::fs::read_to_string(&file_path).unwrap();
    assert!(file_content.contains("Persistent Task"));
    // JSON pretty-print adds a space after the colon
    assert!(file_content.contains("\"status\": \"pending\"") || file_content.contains("\"status\":\"pending\""));
}

#[tokio::test]
async fn test_todo_session_isolation() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path.clone());

    // Create todo list for session 1
    let ctx1 = create_test_context("session1", fixture.path());
    let params1 = json!({
        "todos": [
            {
                "content": "Session 1 Task",
                "status": "pending",
                "active_form": "Working on Session 1 Task"
            }
        ]
    });
    tool.execute(params1, &ctx1).await.unwrap();

    // Create todo list for session 2
    let ctx2 = create_test_context("session2", fixture.path());
    let params2 = json!({
        "todos": [
            {
                "content": "Session 2 Task",
                "status": "pending",
                "active_form": "Working on Session 2 Task"
            }
        ]
    });
    tool.execute(params2, &ctx2).await.unwrap();

    // Verify both files exist
    assert!(storage_path.join("session1.json").exists());
    assert!(storage_path.join("session2.json").exists());

    // Verify content is isolated
    let content1 = std::fs::read_to_string(storage_path.join("session1.json")).unwrap();
    let content2 = std::fs::read_to_string(storage_path.join("session2.json")).unwrap();

    assert!(content1.contains("Session 1 Task"));
    assert!(!content1.contains("Session 2 Task"));

    assert!(content2.contains("Session 2 Task"));
    assert!(!content2.contains("Session 1 Task"));
}

#[tokio::test]
async fn test_todo_update_task_status() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path.clone());
    let ctx = create_test_context("session1", fixture.path());

    // Create initial todo with pending status
    let params1 = json!({
        "todos": [
            {
                "content": "Task to update",
                "status": "pending",
                "active_form": "Working on task"
            }
        ]
    });
    tool.execute(params1, &ctx).await.unwrap();

    // Update to in_progress
    let params2 = json!({
        "todos": [
            {
                "content": "Task to update",
                "status": "in_progress",
                "active_form": "Working on task"
            }
        ]
    });
    let result = tool.execute(params2, &ctx).await.unwrap();

    assert_eq!(result.metadata.get("in_progress").unwrap(), &json!(1));
    assert_eq!(result.metadata.get("pending").unwrap(), &json!(0));

    // Update to completed
    let params3 = json!({
        "todos": [
            {
                "content": "Task to update",
                "status": "completed",
                "active_form": "Working on task"
            }
        ]
    });
    let result = tool.execute(params3, &ctx).await.unwrap();

    assert_eq!(result.metadata.get("completed").unwrap(), &json!(1));
    assert_eq!(result.metadata.get("in_progress").unwrap(), &json!(0));
}

#[tokio::test]
async fn test_todo_empty_list() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path);
    let ctx = create_test_context("session1", fixture.path());

    let params = json!({
        "todos": []
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_tasks").unwrap(), &json!(0));
}

#[tokio::test]
async fn test_todo_large_list() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path);
    let ctx = create_test_context("session1", fixture.path());

    // Create a list with 20 tasks
    let mut todos = Vec::new();
    for i in 1..=20 {
        todos.push(json!({
            "content": format!("Task {}", i),
            "status": if i == 10 { "in_progress" } else if i <= 5 { "completed" } else { "pending" },
            "active_form": format!("Working on Task {}", i)
        }));
    }

    let params = json!({
        "todos": todos
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("total_tasks").unwrap(), &json!(20));
    assert_eq!(output.metadata.get("completed").unwrap(), &json!(5));
    assert_eq!(output.metadata.get("in_progress").unwrap(), &json!(1));
    assert_eq!(output.metadata.get("pending").unwrap(), &json!(14));
}

#[tokio::test]
async fn test_todo_output_format() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path);
    let ctx = create_test_context("session1", fixture.path());

    let params = json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "pending",
                "active_form": "Working on Task 1"
            },
            {
                "content": "Task 2",
                "status": "in_progress",
                "active_form": "Currently doing Task 2"
            }
        ]
    });

    let result = tool.execute(params, &ctx).await.unwrap();

    // Verify output format
    assert!(result.output.contains("Task list updated for session"));
    assert!(result.output.contains("Summary:"));
    assert!(result.output.contains("Total:"));
    assert!(result.output.contains("Pending:"));
    assert!(result.output.contains("In Progress:"));
    assert!(result.output.contains("Completed:"));
    assert!(result.output.contains("Current in-progress task:"));
    assert!(result.output.contains("Currently doing Task 2"));
}

#[tokio::test]
async fn test_todo_all_completed() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path);
    let ctx = create_test_context("session1", fixture.path());

    let params = json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "completed",
                "active_form": "Working on Task 1"
            },
            {
                "content": "Task 2",
                "status": "completed",
                "active_form": "Working on Task 2"
            }
        ]
    });

    let result = tool.execute(params, &ctx).await.unwrap();

    assert_eq!(result.metadata.get("completed").unwrap(), &json!(2));
    assert_eq!(result.metadata.get("in_progress").unwrap(), &json!(0));
    assert_eq!(result.metadata.get("pending").unwrap(), &json!(0));

    // Should not show "Current in-progress task" when none exists
    assert!(!result.output.contains("Current in-progress task:"));
}

#[tokio::test]
async fn test_todo_json_structure() {
    let fixture = TestFixture::new();
    let storage_path = fixture.path().join("todos");

    let tool = TodoWriteTool::with_storage_path(storage_path.clone());
    let ctx = create_test_context("session1", fixture.path());

    let params = json!({
        "todos": [
            {
                "content": "Test Task",
                "status": "pending",
                "active_form": "Testing"
            }
        ]
    });

    tool.execute(params, &ctx).await.unwrap();

    // Read and parse the JSON file
    let file_path = storage_path.join("session1.json");
    let file_content = std::fs::read_to_string(&file_path).unwrap();
    let json_data: serde_json::Value = serde_json::from_str(&file_content).unwrap();

    // Verify JSON structure
    assert!(json_data.get("session_id").is_some());
    assert!(json_data.get("tasks").is_some());
    assert!(json_data.get("updated_at").is_some());

    assert_eq!(json_data["session_id"], "session1");

    let tasks = json_data["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);

    let task = &tasks[0];
    assert!(task.get("id").is_some()); // UUID
    assert_eq!(task["content"], "Test Task");
    assert_eq!(task["status"], "pending");
    assert_eq!(task["active_form"], "Testing");
    assert!(task.get("created_at").is_some());
}
