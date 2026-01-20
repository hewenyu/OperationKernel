pub mod base;
pub mod context;
pub mod bash;
pub mod bash_output;
pub mod kill_shell;
pub mod read;
pub mod write;
pub mod grep;
pub mod glob;
pub mod edit;
pub mod todo;
pub mod notebook;
pub mod web_fetch;
pub mod web_search;
pub mod task;
pub mod ask_user_question;
pub mod enter_plan_mode;
pub mod exit_plan_mode;

use base::Tool;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Tool registry - manages all available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new tool registry with all core tools registered
    pub fn new() -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        // Register the core tools
        tools.insert("bash".to_string(), Arc::new(bash::BashTool));
        tools.insert("read".to_string(), Arc::new(read::ReadTool::new()));
        tools.insert("write".to_string(), Arc::new(write::WriteTool));
        tools.insert("grep".to_string(), Arc::new(grep::GrepTool::new()));
        tools.insert("glob".to_string(), Arc::new(glob::GlobTool::new()));

        // Register extended tools (Phase 1)
        tools.insert("edit".to_string(), Arc::new(edit::EditTool));
        tools.insert("todo_write".to_string(), Arc::new(todo::TodoWriteTool::new()));

        // Register file operation tools (Phase 1 - Additional)
        tools.insert("notebook_edit".to_string(), Arc::new(notebook::NotebookEditTool));

        // Register background process tools (Phase 2)
        tools.insert("bash_output".to_string(), Arc::new(bash_output::BashOutputTool));
        tools.insert("kill_shell".to_string(), Arc::new(kill_shell::KillShellTool));

        // Register web integration tools (Phase 3)
        tools.insert("web_fetch".to_string(), Arc::new(web_fetch::WebFetchTool::new()));

        // Register interactive tools (Phase 4)
        tools.insert("ask_user_question".to_string(), Arc::new(ask_user_question::AskUserQuestionTool));
        tools.insert("enter_plan_mode".to_string(), Arc::new(enter_plan_mode::EnterPlanModeTool));
        tools.insert("exit_plan_mode".to_string(), Arc::new(exit_plan_mode::ExitPlanModeTool));

        // Register advanced tools (Phase 5)
        tools.insert("web_search".to_string(), Arc::new(web_search::WebSearchTool::new()));
        // Note: TaskTool will be added dynamically in AgentRunner::new() since it needs LLM client

        Self { tools }
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Get all tool definitions for Claude API
    pub fn list_tool_definitions(&self) -> Vec<serde_json::Value> {
        self.tools
            .values()
            .map(|tool| {
                json!({
                    "name": tool.id(),
                    "description": tool.description(),
                    "input_schema": tool.input_schema(),
                })
            })
            .collect()
    }

    /// Get all tool names
    pub fn list_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Create a tool registry from an existing HashMap of tools
    ///
    /// This is used for creating filtered registries for subagents
    pub fn from_map(tools: HashMap<String, Arc<dyn Tool>>) -> Self {
        Self { tools }
    }

    /// Dynamically insert a tool into the registry
    ///
    /// This is used for adding tools that require runtime dependencies (e.g., TaskTool needs LLM client)
    pub fn insert_tool(&mut self, name: String, tool: Arc<dyn Tool>) {
        self.tools.insert(name, tool);
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
