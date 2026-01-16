use serde::{Deserialize, Serialize};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default station to use
    #[serde(default = "default_station_id")]
    pub default_station: String,

    /// Available LLM stations
    #[serde(default)]
    pub stations: Vec<Station>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_station: "claude".to_string(),
            stations: vec![
                Station {
                    id: "claude".to_string(),
                    name: "Claude 3.5 Sonnet".to_string(),
                    provider: Provider::Anthropic,
                    api_key: "YOUR_API_KEY_HERE".to_string(),
                    api_base: Some("https://api.anthropic.com".to_string()),
                    model: "claude-3-5-sonnet-20241022".to_string(),
                    max_tokens: Some(8192),
                    temperature: Some(1.0),
                },
            ],
        }
    }
}

/// A "station" represents one LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    /// Unique identifier for this station
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Provider type
    pub provider: Provider,

    /// API key
    pub api_key: String,

    /// Optional custom API base URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    /// Model identifier
    pub model: String,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Temperature (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// Supported LLM providers
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    OpenAI,
    Gemini,
}

impl Provider {
    #[allow(dead_code)]
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Provider::Anthropic => "https://api.anthropic.com",
            Provider::OpenAI => "https://api.openai.com",
            Provider::Gemini => "https://generativelanguage.googleapis.com",
        }
    }
}

fn default_station_id() -> String {
    "claude".to_string()
}
