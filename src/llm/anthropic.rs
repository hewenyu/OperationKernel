use crate::config::station::Station;
use crate::llm::types::{Message, StreamChunk};
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
    ) -> Result<Pin<Box<dyn Stream<Item = StreamChunk> + Send>>> {
        let api_base = self
            .station
            .api_base
            .as_deref()
            .unwrap_or("https://api.anthropic.com");

        let url = format!("{}/v1/messages", api_base);

        let request_body = CreateMessageRequest {
            model: self.station.model.clone(),
            messages,
            max_tokens: self.station.max_tokens.unwrap_or(8192),
            temperature: self.station.temperature,
            stream: true,
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
            .context("Failed to send request to Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("API request failed ({}): {}", status, error_text);
        }

        let stream = response
            .bytes_stream()
            .eventsource()
            .map(|event| match event {
                Ok(event) => {
                    if event.event == "content_block_delta" {
                        // Parse the delta content
                        if let Ok(delta) = serde_json::from_str::<ContentBlockDelta>(&event.data) {
                            if let Some(text) = delta.delta.text {
                                return StreamChunk::Text(text);
                            }
                        }
                    } else if event.event == "message_stop" {
                        return StreamChunk::Done;
                    }
                    // Ignore other event types
                    StreamChunk::Text(String::new())
                }
                Err(e) => StreamChunk::Error(e.to_string()),
            })
            .filter(|chunk| {
                // Filter out empty text chunks
                futures::future::ready(!matches!(chunk, StreamChunk::Text(s) if s.is_empty()))
            });

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
}

/// Content block delta event
#[derive(Debug, Deserialize)]
struct ContentBlockDelta {
    delta: Delta,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(rename = "type")]
    _type: String,
    text: Option<String>,
}
