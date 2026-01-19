pub mod base;
pub mod context;
pub mod bash;
pub mod read;
pub mod write;

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

        // Register the three core tools
        tools.insert("bash".to_string(), Arc::new(bash::BashTool));
        tools.insert("read".to_string(), Arc::new(read::ReadTool::new()));
        tools.insert("write".to_string(), Arc::new(write::WriteTool));

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
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
