use ok::tool::ToolRegistry;

#[test]
fn test_edit_tool_registered() {
    let registry = ToolRegistry::new();
    let tool = registry.get("edit");

    assert!(tool.is_some(), "Edit tool should be registered");

    let tool = tool.unwrap();
    assert_eq!(tool.id(), "edit");
    assert!(tool.description().contains("Edit"));
}

#[test]
fn test_todo_write_tool_registered() {
    let registry = ToolRegistry::new();
    let tool = registry.get("todo_write");

    assert!(tool.is_some(), "TodoWrite tool should be registered");

    let tool = tool.unwrap();
    assert_eq!(tool.id(), "todo_write");
    assert!(tool.description().contains("task"));
}

#[test]
fn test_all_tools_registered() {
    let registry = ToolRegistry::new();

    // Core tools
    assert!(registry.get("bash").is_some(), "Bash tool should be registered");
    assert!(registry.get("read").is_some(), "Read tool should be registered");
    assert!(registry.get("write").is_some(), "Write tool should be registered");
    assert!(registry.get("grep").is_some(), "Grep tool should be registered");
    assert!(registry.get("glob").is_some(), "Glob tool should be registered");

    // Phase 1 extended tools
    assert!(registry.get("edit").is_some(), "Edit tool should be registered");
    assert!(registry.get("todo_write").is_some(), "TodoWrite tool should be registered");

    // Additional tools
    assert!(registry.get("notebook_edit").is_some(), "NotebookEdit tool should be registered");
    assert!(registry.get("bash_output").is_some(), "BashOutput tool should be registered");
    assert!(registry.get("kill_shell").is_some(), "KillShell tool should be registered");
    assert!(registry.get("web_fetch").is_some(), "WebFetch tool should be registered");

    // Interactive tools (Phase 4)
    assert!(registry.get("ask_user_question").is_some(), "AskUserQuestion tool should be registered");
    assert!(registry.get("enter_plan_mode").is_some(), "EnterPlanMode tool should be registered");
    assert!(registry.get("exit_plan_mode").is_some(), "ExitPlanMode tool should be registered");

    // Phase 5 advanced tools
    assert!(registry.get("web_search").is_some(), "WebSearch tool should be registered");
    // Note: TaskTool is registered dynamically in AgentRunner, not in ToolRegistry::new()

    // Total count should be 15
    let definitions = registry.list_tool_definitions();
    assert_eq!(definitions.len(), 15, "Should have exactly 15 tools registered");
}

#[test]
fn test_tool_schemas_valid() {
    let registry = ToolRegistry::new();

    // Verify Edit tool schema
    let edit = registry.get("edit").unwrap();
    let schema = edit.input_schema();
    assert!(schema.get("type").is_some());
    assert!(schema.get("properties").is_some());
    assert!(schema.get("required").is_some());

    // Verify TodoWrite tool schema
    let todo = registry.get("todo_write").unwrap();
    let schema = todo.input_schema();
    assert!(schema.get("type").is_some());
    assert!(schema.get("properties").is_some());
    assert!(schema.get("required").is_some());
}
