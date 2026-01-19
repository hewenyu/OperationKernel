use ok::llm::types::{ContentBlock, Message, ToolUse};
use serde_json::json;

#[test]
fn serializes_user_message_with_text_content() {
    let msg = Message::user("hi");
    let value = serde_json::to_value(msg).unwrap();
    assert_eq!(value, json!({ "role": "user", "content": "hi" }));
}

#[test]
fn serializes_assistant_message_with_text_content() {
    let msg = Message::assistant("ok");
    let value = serde_json::to_value(msg).unwrap();
    assert_eq!(value, json!({ "role": "assistant", "content": "ok" }));
}

#[test]
fn serializes_tool_result_message_as_blocks() {
    let msg = Message::user_with_tool_result("toolu_123".to_string(), "output".to_string());
    let value = serde_json::to_value(msg).unwrap();
    assert_eq!(
        value,
        json!({
            "role": "user",
            "content": [
                { "type": "tool_result", "tool_use_id": "toolu_123", "content": "output" }
            ]
        })
    );
}

#[test]
fn serializes_tool_result_message_with_is_error() {
    let msg = Message::user_with_tool_result_detailed(
        "toolu_123".to_string(),
        "output".to_string(),
        Some(true),
    );
    let value = serde_json::to_value(msg).unwrap();
    assert_eq!(
        value,
        json!({
            "role": "user",
            "content": [
                { "type": "tool_result", "tool_use_id": "toolu_123", "content": "output", "is_error": true }
            ]
        })
    );
}

#[test]
fn serializes_assistant_blocks_with_tool_use() {
    let tool_use = ToolUse {
        id: "toolu_abc".to_string(),
        name: "bash".to_string(),
        input: json!({ "command": "echo hi" }),
    };

    let msg = Message::assistant_with_blocks(vec![
        ContentBlock::Text {
            text: "Running command".to_string(),
        },
        ContentBlock::ToolUse(tool_use),
    ]);

    let value = serde_json::to_value(msg).unwrap();
    assert_eq!(
        value,
        json!({
            "role": "assistant",
            "content": [
                { "type": "text", "text": "Running command" },
                { "type": "tool_use", "id": "toolu_abc", "name": "bash", "input": { "command": "echo hi" } }
            ]
        })
    );
}
