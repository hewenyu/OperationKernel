use crate::config::station::Station;
use crate::llm::types::{Message, StreamChunk, ToolUse};
use anyhow::{Context, Result};
use eventsource_stream::Eventsource;
use futures::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio_stream::Stream;

/// Anthropic API client
#[derive(Clone)]
pub struct AnthropicClient {
    client: Client,
    station: Station,
}

impl AnthropicClient {
    pub fn new(station: Station) -> Self {
        Self {
            client: Client::new(),
            station,
        }
    }

    /// Create a streaming chat completion
    pub async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamChunk> + Send>>> {
        let api_base = self
            .station
            .api_base
            .as_deref()
            .unwrap_or("https://api.anthropic.com");

        let url = format!("{}/v1/messages", api_base);

        tracing::debug!(
            api_base = %api_base,
            model = %self.station.model,
            message_count = messages.len(),
            tool_count = tools.as_ref().map(|t| t.len()).unwrap_or(0),
            "anthropic stream_chat request"
        );

        let request_body = CreateMessageRequest {
            model: self.station.model.clone(),
            messages,
            max_tokens: self.station.max_tokens.unwrap_or(8192),
            temperature: self.station.temperature,
            stream: true,
            tools,
        };

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.station.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Network error: Failed to send request to Anthropic API. Check your internet connection.")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            tracing::warn!(
                status = %status,
                error = %crate::logging::redact_secrets(&error_text),
                "anthropic api returned error"
            );

            // Provide more specific error messages based on status code
            let error_msg = match status.as_u16() {
                401 => format!("Unauthorized (401): Invalid or missing API key. Please check your API key in ~/.config/ok/config.toml\n\nDetails: {}", error_text),
                429 => format!("Rate Limit Exceeded (429): You've made too many requests. Please wait a moment and try again.\n\nDetails: {}", error_text),
                400 => format!("Bad Request (400): The request was invalid. Please check your input.\n\nDetails: {}", error_text),
                500..=599 => format!("Server Error ({}): The Anthropic API is experiencing issues. Please try again later.\n\nDetails: {}", status, error_text),
                _ => format!("API request failed ({}): {}", status, error_text),
            };

            anyhow::bail!(error_msg);
        }

        let stream = response
            .bytes_stream()
            .eventsource()
            .scan(StreamState::default(), |state, event| {
                let out: Option<StreamChunk> = match event {
                    Err(e) => Some(StreamChunk::Error(e.to_string())),
                    Ok(event) => match event.event.as_str() {
                        "content_block_start" => {
                            let Ok(start) = serde_json::from_str::<ContentBlockStart>(&event.data)
                            else {
                                return futures::future::ready(Some(None));
                            };

                            if start.content_block.block_type == "tool_use" {
                                let (Some(id), Some(name)) =
                                    (start.content_block.id, start.content_block.name)
                                else {
                                    return futures::future::ready(Some(None));
                                };

                                tracing::debug!(tool_id = %id, tool_name = %name, "anthropic tool_use start");

                                state.pending_tool = Some(PendingToolUse {
                                    id,
                                    name,
                                    input: start.content_block.input,
                                    input_json: String::new(),
                                });
                            }
                            None
                        }
                        "content_block_delta" => {
                            let Ok(delta) = serde_json::from_str::<ContentBlockDelta>(&event.data)
                            else {
                                return futures::future::ready(Some(None));
                            };

                            match delta.delta.delta_type.as_str() {
                                "text_delta" => delta.delta.text.map(StreamChunk::Text),
                                "input_json_delta" => {
                                    if let (Some(pending), Some(partial)) = (
                                        state.pending_tool.as_mut(),
                                        delta.delta.partial_json,
                                    ) {
                                        pending.input_json.push_str(&partial);
                                    }
                                    None
                                }
                                _ => None,
                            }
                        }
                        "content_block_stop" => {
                            let Some(pending) = state.pending_tool.take() else {
                                return futures::future::ready(Some(None));
                            };

                            let input = if pending.input_json.trim().is_empty() {
                                pending.input
                            } else {
                                match serde_json::from_str::<serde_json::Value>(
                                    &pending.input_json,
                                ) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        return futures::future::ready(Some(Some(StreamChunk::Error(
                                            format!(
                                                "Failed to parse tool input JSON for '{}': {e}",
                                                pending.name
                                            ),
                                        ))));
                                    }
                                }
                            };

                            Some(StreamChunk::ToolUse(ToolUse {
                                id: pending.id,
                                name: pending.name,
                                input,
                            }))
                        }
                        "message_stop" => Some(StreamChunk::Done),
                        _ => None,
                    },
                };

                futures::future::ready(Some(out))
            })
            .filter_map(|chunk| futures::future::ready(chunk));

        Ok(Box::pin(stream))
    }
}

/// Request body for creating a message
#[derive(Debug, Serialize)]
struct CreateMessageRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

/// Content block start event (for tool_use)
#[derive(Debug, Deserialize)]
struct ContentBlockStart {
    content_block: ContentBlockData,
}

#[derive(Debug, Deserialize)]
struct ContentBlockData {
    #[serde(rename = "type")]
    block_type: String,
    id: Option<String>,
    name: Option<String>,
    #[serde(default = "default_tool_input")]
    input: serde_json::Value,
}

/// Content block delta event
#[derive(Debug, Deserialize)]
struct ContentBlockDelta {
    delta: Delta,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
    partial_json: Option<String>,
}

fn default_tool_input() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Default)]
struct StreamState {
    pending_tool: Option<PendingToolUse>,
}

struct PendingToolUse {
    id: String,
    name: String,
    input: serde_json::Value,
    input_json: String,
}
