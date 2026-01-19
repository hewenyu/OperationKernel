use anyhow::{anyhow, Result};

/// Types of specialized subagents
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentType {
    /// General-purpose agent with access to all tools
    GeneralPurpose,
    /// Codebase exploration specialist (Read, Grep, Glob, Bash read-only)
    Explore,
    /// Implementation planning architect (Read, Grep, Glob, Bash read-only)
    Plan,
    /// Fast Bash execution specialist
    Bash,
}

impl std::str::FromStr for SubagentType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "general-purpose" => Ok(SubagentType::GeneralPurpose),
            "Explore" => Ok(SubagentType::Explore),
            "Plan" => Ok(SubagentType::Plan),
            "Bash" => Ok(SubagentType::Bash),
            _ => Err(anyhow!("Unknown subagent type: {}", s)),
        }
    }
}

impl std::fmt::Display for SubagentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubagentType::GeneralPurpose => write!(f, "general-purpose"),
            SubagentType::Explore => write!(f, "Explore"),
            SubagentType::Plan => write!(f, "Plan"),
            SubagentType::Bash => write!(f, "Bash"),
        }
    }
}

/// Configuration for a subagent including available tools and custom system prompt
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    pub name: String,
    pub description: String,
    pub available_tools: Vec<String>,
    pub system_prompt: Option<String>,
}

impl SubagentConfig {
    /// Get configuration for a specific subagent type
    pub fn for_type(subagent_type: &SubagentType) -> Self {
        match subagent_type {
            SubagentType::GeneralPurpose => Self {
                name: "general-purpose".to_string(),
                description: "General-purpose agent for complex multi-step tasks".to_string(),
                available_tools: vec!["*".to_string()], // All tools
                system_prompt: None, // Use default system prompt
            },
            SubagentType::Explore => Self {
                name: "Explore".to_string(),
                description: "Codebase exploration specialist".to_string(),
                available_tools: vec![
                    "read".to_string(),
                    "grep".to_string(),
                    "glob".to_string(),
                    "bash".to_string(),
                ],
                system_prompt: Some(
                    "You are a codebase exploration specialist. Your role is to:\n\
                     - Explore and understand codebases systematically\n\
                     - Find relevant files, functions, and patterns\n\
                     - Provide clear summaries of code structure and organization\n\
                     - Answer questions about code architecture and implementation\n\n\
                     Available tools: Read, Grep, Glob, Bash (read-only)\n\
                     Focus on thorough exploration and clear explanations."
                        .to_string(),
                ),
            },
            SubagentType::Plan => Self {
                name: "Plan".to_string(),
                description: "Implementation planning architect".to_string(),
                available_tools: vec![
                    "read".to_string(),
                    "grep".to_string(),
                    "glob".to_string(),
                    "bash".to_string(),
                ],
                system_prompt: Some(
                    "You are an implementation planning architect. Your role is to:\n\
                     - Analyze the codebase to understand existing patterns\n\
                     - Design implementation approaches for new features\n\
                     - Identify files that need to be created or modified\n\
                     - Consider architectural trade-offs and risks\n\
                     - Create step-by-step implementation plans\n\n\
                     Available tools: Read, Grep, Glob, Bash (read-only)\n\
                     Focus on thorough analysis and detailed planning."
                        .to_string(),
                ),
            },
            SubagentType::Bash => Self {
                name: "Bash".to_string(),
                description: "Fast Bash execution specialist".to_string(),
                available_tools: vec!["bash".to_string(), "bash_output".to_string()],
                system_prompt: Some(
                    "You are a Bash execution specialist. Your role is to:\n\
                     - Execute shell commands quickly and efficiently\n\
                     - Handle command output and errors appropriately\n\
                     - Provide clear summaries of command results\n\n\
                     Available tools: Bash, BashOutput\n\
                     Focus on quick execution and clear reporting."
                        .to_string(),
                ),
            },
        }
    }

    /// Check if a tool is available for this subagent
    pub fn is_tool_available(&self, tool_name: &str) -> bool {
        self.available_tools.contains(&"*".to_string())
            || self.available_tools.contains(&tool_name.to_string())
    }

    /// Get filtered tool list (for tool registry filtering)
    pub fn get_filtered_tools(&self) -> Vec<String> {
        self.available_tools.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_type_parsing() {
        assert_eq!(
            "general-purpose".parse::<SubagentType>().unwrap(),
            SubagentType::GeneralPurpose
        );
        assert_eq!(
            "Explore".parse::<SubagentType>().unwrap(),
            SubagentType::Explore
        );
        assert_eq!(
            "Plan".parse::<SubagentType>().unwrap(),
            SubagentType::Plan
        );
        assert_eq!(
            "Bash".parse::<SubagentType>().unwrap(),
            SubagentType::Bash
        );

        assert!("invalid".parse::<SubagentType>().is_err());
    }

    #[test]
    fn test_general_purpose_has_all_tools() {
        let config = SubagentConfig::for_type(&SubagentType::GeneralPurpose);
        assert!(config.is_tool_available("any_tool"));
        assert!(config.is_tool_available("read"));
        assert!(config.is_tool_available("write"));
    }

    #[test]
    fn test_explore_limited_tools() {
        let config = SubagentConfig::for_type(&SubagentType::Explore);
        assert!(config.is_tool_available("read"));
        assert!(config.is_tool_available("grep"));
        assert!(config.is_tool_available("glob"));
        assert!(config.is_tool_available("bash"));
        assert!(!config.is_tool_available("write"));
        assert!(!config.is_tool_available("edit"));
    }

    #[test]
    fn test_plan_has_custom_system_prompt() {
        let config = SubagentConfig::for_type(&SubagentType::Plan);
        assert!(config.system_prompt.is_some());
        assert!(config
            .system_prompt
            .unwrap()
            .contains("implementation planning"));
    }

    #[test]
    fn test_bash_specialist_tools() {
        let config = SubagentConfig::for_type(&SubagentType::Bash);
        assert!(config.is_tool_available("bash"));
        assert!(config.is_tool_available("bash_output"));
        assert!(!config.is_tool_available("read"));
    }
}
