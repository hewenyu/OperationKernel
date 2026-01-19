use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use uuid::Uuid;

/// TodoWrite tool - manages task lists with session isolation
pub struct TodoWriteTool {
    storage_path: PathBuf,
}

impl TodoWriteTool {
    /// Create a new TodoWrite tool with default storage location
    pub fn new() -> Self {
        let storage_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ok")
            .join("todos");

        Self { storage_path }
    }

    /// Create a TodoWrite tool with custom storage path (for testing)
    #[cfg(test)]
    pub fn with_storage_path(storage_path: PathBuf) -> Self {
        Self { storage_path }
    }

    /// Load existing todo list for a session, or create a new one
    async fn load_or_create(&self, session_id: &str) -> Result<TodoList, ToolError> {
        let filepath = self.storage_path.join(format!("{}.json", session_id));

        if filepath.exists() {
            let content = tokio::fs::read_to_string(&filepath)
                .await
                .map_err(|e| ToolError::Other(e.into()))?;

            serde_json::from_str(&content)
                .map_err(|e| ToolError::InvalidParams(format!("Failed to parse todo list: {}", e)))
        } else {
            Ok(TodoList {
                session_id: session_id.to_string(),
                tasks: Vec::new(),
                updated_at: chrono::Utc::now(),
            })
        }
    }

    /// Save todo list to file
    async fn save(&self, todo_list: &TodoList) -> Result<(), ToolError> {
        // Ensure directory exists
        tokio::fs::create_dir_all(&self.storage_path)
            .await
            .map_err(|e| ToolError::Other(e.into()))?;

        let filepath = self
            .storage_path
            .join(format!("{}.json", todo_list.session_id));

        let json = serde_json::to_string_pretty(todo_list)
            .map_err(|e| ToolError::Other(e.into()))?;

        tokio::fs::write(&filepath, json)
            .await
            .map_err(|e| ToolError::Other(e.into()))?;

        Ok(())
    }

    /// Generate summary output for the todo list
    fn generate_summary(todo_list: &TodoList) -> String {
        let pending = todo_list.tasks.iter().filter(|t| t.status == TaskStatus::Pending).count();
        let in_progress = todo_list.tasks.iter().filter(|t| t.status == TaskStatus::InProgress).count();
        let completed = todo_list.tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();

        let mut output = format!(
            "Task list updated for session: {}\n\n",
            todo_list.session_id
        );

        output.push_str("Summary:\n");
        output.push_str(&format!("  Total: {} task(s)\n", todo_list.tasks.len()));
        output.push_str(&format!("  Pending: {} task(s)\n", pending));
        output.push_str(&format!("  In Progress: {} task(s)\n", in_progress));
        output.push_str(&format!("  Completed: {} task(s)\n\n", completed));

        // Show current in-progress task
        if let Some(task) = todo_list.tasks.iter().find(|t| t.status == TaskStatus::InProgress) {
            output.push_str("Current in-progress task:\n");
            output.push_str(&format!("  → {}\n", task.active_form));
        }

        output
    }
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistent todo list for a session
#[derive(Debug, Serialize, Deserialize, Clone)]
struct TodoList {
    session_id: String,
    tasks: Vec<Task>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// Individual task
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Task {
    id: String,
    content: String,
    status: TaskStatus,
    active_form: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Task status enum
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            _ => Err(format!("Invalid task status: {}", s)),
        }
    }
}

/// Input parameters for TodoWrite
#[derive(Debug, Deserialize)]
struct TodoParams {
    todos: Vec<TodoInput>,
}

/// Individual todo item from input
#[derive(Debug, Deserialize)]
struct TodoInput {
    content: String,
    status: String,
    active_form: String,
}

#[async_trait::async_trait]
impl Tool for TodoWriteTool {
    fn id(&self) -> &str {
        "todo_write"
    }

    fn description(&self) -> &str {
        "Manage task lists with status tracking. Supports three states: pending, in_progress, \
         and completed. Enforces the constraint that at most one task can be in_progress at \
         a time. Tasks are persisted per session and survive application restarts."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "List of tasks to set (replaces the entire task list)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "Task description (imperative form, e.g., 'Implement feature X')"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "Task status"
                            },
                            "active_form": {
                                "type": "string",
                                "description": "Present continuous form shown during execution (e.g., 'Implementing feature X')"
                            }
                        },
                        "required": ["content", "status", "active_form"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: TodoParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(
            session_id = %ctx.session_id,
            num_todos = params.todos.len(),
            "tool todo_write start"
        );

        // 1. Validate status strings and count in_progress tasks
        let mut in_progress_count = 0;
        for todo in &params.todos {
            // Validate status string
            todo.status
                .parse::<TaskStatus>()
                .map_err(|e| ToolError::InvalidParams(e))?;

            if todo.status == "in_progress" {
                in_progress_count += 1;
            }
        }

        // 2. Enforce constraint: at most one in_progress task
        if in_progress_count > 1 {
            return Err(ToolError::InvalidParams(format!(
                "Only one task can be in_progress, found {}",
                in_progress_count
            )));
        }

        // 3. Load existing todo list (not used in current implementation, but available for future merging logic)
        let _existing = self.load_or_create(&ctx.session_id).await?;

        // 4. Convert input todos to Task objects
        let now = chrono::Utc::now();
        let tasks: Vec<Task> = params
            .todos
            .iter()
            .map(|todo| Task {
                id: Uuid::new_v4().to_string(),
                content: todo.content.clone(),
                status: todo.status.parse().unwrap(), // Safe because we validated above
                active_form: todo.active_form.clone(),
                created_at: now,
            })
            .collect();

        // 5. Create new todo list (replaces existing)
        let todo_list = TodoList {
            session_id: ctx.session_id.clone(),
            tasks,
            updated_at: now,
        };

        // 6. Save to disk
        self.save(&todo_list).await?;

        // 7. Generate summary output
        let output = Self::generate_summary(&todo_list);

        tracing::debug!(
            session_id = %ctx.session_id,
            total_tasks = todo_list.tasks.len(),
            "tool todo_write done"
        );

        // 8. Return result
        let pending = todo_list.tasks.iter().filter(|t| t.status == TaskStatus::Pending).count();
        let in_progress = todo_list.tasks.iter().filter(|t| t.status == TaskStatus::InProgress).count();
        let completed = todo_list.tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();

        Ok(ToolResult::new("Todo list updated", output)
            .with_metadata("session_id", json!(ctx.session_id))
            .with_metadata("total_tasks", json!(todo_list.tasks.len()))
            .with_metadata("pending", json!(pending))
            .with_metadata("in_progress", json!(in_progress))
            .with_metadata("completed", json!(completed)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_context(session_id: &str) -> ToolContext {
        ToolContext {
            session_id: session_id.to_string(),
            message_id: "test-message".to_string(),
            agent: "test-agent".to_string(),
            working_dir: PathBuf::from("/tmp"),
        }
    }

    #[tokio::test]
    async fn test_create_todo_list() {
        let temp_dir = tempdir().unwrap();
        let tool = TodoWriteTool::with_storage_path(temp_dir.path().to_path_buf());

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

        let ctx = create_test_context("test-session");
        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.metadata.get("total_tasks").unwrap(), &json!(2));
        assert_eq!(result.metadata.get("pending").unwrap(), &json!(1));
        assert_eq!(result.metadata.get("completed").unwrap(), &json!(1));

        // Verify file exists
        let filepath = temp_dir.path().join("test-session.json");
        assert!(filepath.exists());
    }

    #[tokio::test]
    async fn test_single_in_progress_constraint() {
        let temp_dir = tempdir().unwrap();
        let tool = TodoWriteTool::with_storage_path(temp_dir.path().to_path_buf());

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

        let ctx = create_test_context("test-session");
        let result = tool.execute(params, &ctx).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("Only one task can be in_progress"));
            }
            _ => panic!("Expected InvalidParams error"),
        }
    }

    #[tokio::test]
    async fn test_invalid_status() {
        let temp_dir = tempdir().unwrap();
        let tool = TodoWriteTool::with_storage_path(temp_dir.path().to_path_buf());

        let params = json!({
            "todos": [
                {
                    "content": "Task 1",
                    "status": "invalid_status",
                    "active_form": "Working on Task 1"
                }
            ]
        });

        let ctx = create_test_context("test-session");
        let result = tool.execute(params, &ctx).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = tempdir().unwrap();
        let tool = TodoWriteTool::with_storage_path(temp_dir.path().to_path_buf());

        // Create initial todo list
        let params = json!({
            "todos": [
                {
                    "content": "Task 1",
                    "status": "pending",
                    "active_form": "Working on Task 1"
                }
            ]
        });

        let ctx = create_test_context("test-session");
        tool.execute(params, &ctx).await.unwrap();

        // Load it back
        let loaded = tool.load_or_create("test-session").await.unwrap();
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].content, "Task 1");
    }

    #[tokio::test]
    async fn test_multiple_sessions() {
        let temp_dir = tempdir().unwrap();
        let tool = TodoWriteTool::with_storage_path(temp_dir.path().to_path_buf());

        // Create todo list for session 1
        let params1 = json!({
            "todos": [
                {
                    "content": "Session 1 Task",
                    "status": "pending",
                    "active_form": "Working on Session 1 Task"
                }
            ]
        });
        let ctx1 = create_test_context("session-1");
        tool.execute(params1, &ctx1).await.unwrap();

        // Create todo list for session 2
        let params2 = json!({
            "todos": [
                {
                    "content": "Session 2 Task",
                    "status": "pending",
                    "active_form": "Working on Session 2 Task"
                }
            ]
        });
        let ctx2 = create_test_context("session-2");
        tool.execute(params2, &ctx2).await.unwrap();

        // Verify both exist and are isolated
        let list1 = tool.load_or_create("session-1").await.unwrap();
        let list2 = tool.load_or_create("session-2").await.unwrap();

        assert_eq!(list1.tasks.len(), 1);
        assert_eq!(list2.tasks.len(), 1);
        assert_eq!(list1.tasks[0].content, "Session 1 Task");
        assert_eq!(list2.tasks[0].content, "Session 2 Task");
    }

    #[tokio::test]
    async fn test_update_task_status() {
        let temp_dir = tempdir().unwrap();
        let tool = TodoWriteTool::with_storage_path(temp_dir.path().to_path_buf());

        // Create initial todo
        let params1 = json!({
            "todos": [
                {
                    "content": "Task 1",
                    "status": "pending",
                    "active_form": "Working on Task 1"
                }
            ]
        });
        let ctx = create_test_context("test-session");
        tool.execute(params1, &ctx).await.unwrap();

        // Update to in_progress
        let params2 = json!({
            "todos": [
                {
                    "content": "Task 1",
                    "status": "in_progress",
                    "active_form": "Working on Task 1"
                }
            ]
        });
        let result = tool.execute(params2, &ctx).await.unwrap();

        assert_eq!(result.metadata.get("in_progress").unwrap(), &json!(1));
        assert_eq!(result.metadata.get("pending").unwrap(), &json!(0));
    }
}
