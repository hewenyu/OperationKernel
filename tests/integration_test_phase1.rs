use ok::agent::AgentRunner;
use ok::config::station::{Provider, Station};
use ok::llm::anthropic::AnthropicClient;

fn create_test_agent_runner() -> AgentRunner {
    let station = Station {
        id: "test".to_string(),
        name: "Test Station".to_string(),
        provider: Provider::Anthropic,
        model: "claude-3-sonnet-20240229".to_string(),
        api_key: "test-key".to_string(),
        api_base: None,
        max_tokens: Some(1024),
        temperature: Some(1.0),
    };

    let llm_client = AnthropicClient::new(station);
    AgentRunner::new(llm_client)
}

#[test]
fn test_phase1_tools_registered() {
    let agent = create_test_agent_runner();
    let registry = agent.tool_registry();

    // Test that WebSearch tool is registered
    assert!(
        registry.get("web_search").is_some(),
        "WebSearch tool should be registered"
    );

    // Test that Task tool is registered
    assert!(
        registry.get("task").is_some(),
        "Task tool should be registered"
    );

    // Test that existing tools are still available
    assert!(
        registry.get("read").is_some(),
        "Read tool should still be available"
    );
    assert!(
        registry.get("write").is_some(),
        "Write tool should still be available"
    );
    assert!(
        registry.get("bash").is_some(),
        "Bash tool should still be available"
    );
}

#[test]
fn test_tool_list_includes_new_tools() {
    let agent = create_test_agent_runner();
    let registry = agent.tool_registry();

    let tool_names = registry.list_names();

    // Check that Phase 1 tools are in the list
    assert!(
        tool_names.contains(&"web_search".to_string()),
        "web_search should be in tool list"
    );
    assert!(
        tool_names.contains(&"task".to_string()),
        "task should be in tool list"
    );

    // Verify total number of tools (should be original + 2 new ones)
    // Original tools: bash, read, write, grep, glob, edit, todo_write, notebook_edit,
    //                 bash_output, kill_shell, web_fetch, ask_user_question,
    //                 enter_plan_mode, exit_plan_mode
    // New tools: web_search, task
    // Total: 16 tools
    assert!(
        tool_names.len() >= 16,
        "Should have at least 16 tools registered, got {}",
        tool_names.len()
    );
}

#[test]
fn test_tool_definitions_include_new_tools() {
    let agent = create_test_agent_runner();
    let registry = agent.tool_registry();

    let definitions = registry.list_tool_definitions();

    // Find web_search in definitions
    let web_search_def = definitions
        .iter()
        .find(|def| def["name"] == "web_search");
    assert!(
        web_search_def.is_some(),
        "web_search should be in tool definitions"
    );

    // Find task in definitions
    let task_def = definitions.iter().find(|def| def["name"] == "task");
    assert!(task_def.is_some(), "task should be in tool definitions");

    // Verify web_search has required fields
    if let Some(def) = web_search_def {
        assert!(
            def["description"].is_string(),
            "web_search should have description"
        );
        assert!(
            def["input_schema"].is_object(),
            "web_search should have input_schema"
        );
        assert_eq!(
            def["input_schema"]["type"], "object",
            "web_search input_schema should be object type"
        );
        assert!(
            def["input_schema"]["properties"]["query"].is_object(),
            "web_search should have query parameter"
        );
    }

    // Verify task has required fields
    if let Some(def) = task_def {
        assert!(def["description"].is_string(), "task should have description");
        assert!(
            def["input_schema"].is_object(),
            "task should have input_schema"
        );
        assert!(
            def["input_schema"]["properties"]["subagent_type"].is_object(),
            "task should have subagent_type parameter"
        );
    }
}

#[test]
fn test_web_search_tool_properties() {
    let agent = create_test_agent_runner();
    let registry = agent.tool_registry();

    let tool = registry.get("web_search").expect("web_search should exist");

    assert_eq!(tool.id(), "web_search");
    assert!(!tool.description().is_empty());

    let schema = tool.input_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["query"].is_object());
    assert!(schema["properties"]["allowed_domains"].is_object());
    assert!(schema["properties"]["blocked_domains"].is_object());
    assert_eq!(schema["required"], serde_json::json!(["query"]));
}

#[test]
fn test_task_tool_properties() {
    let agent = create_test_agent_runner();
    let registry = agent.tool_registry();

    let tool = registry.get("task").expect("task should exist");

    assert_eq!(tool.id(), "task");
    assert!(!tool.description().is_empty());

    let schema = tool.input_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["description"].is_object());
    assert!(schema["properties"]["prompt"].is_object());
    assert!(schema["properties"]["subagent_type"].is_object());
    assert!(schema["properties"]["model"].is_object());

    // Verify subagent_type enum
    let subagent_enum = &schema["properties"]["subagent_type"]["enum"];
    assert!(subagent_enum.is_array());
    let enum_values = subagent_enum.as_array().unwrap();
    assert!(enum_values.contains(&serde_json::json!("Explore")));
    assert!(enum_values.contains(&serde_json::json!("Plan")));

    assert_eq!(
        schema["required"],
        serde_json::json!(["description", "prompt", "subagent_type"])
    );
}
