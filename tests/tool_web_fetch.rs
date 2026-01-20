//! Integration tests for the web_fetch tool
//!
//! Note: These tests are real network tests and are configured via `tests/config.toml`.
//! If your environment is offline or blocks outbound HTTP(S), set `web_fetch.enabled = false`.

mod common;

use common::TestFixture;
use ok::tool::{base::*, web_fetch::WebFetchTool};
use serde_json::json;
use std::sync::Arc;

/// Helper to create a tool context for testing
fn create_test_context(working_dir: std::path::PathBuf) -> ToolContext {
    ToolContext::new(
        "test_session",
        "test_msg",
        "test_station",
        working_dir,
        Arc::new(ok::process::BackgroundShellManager::new()),
    )
}

#[tokio::test]
async fn test_web_fetch_http_upgrade() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // Try to fetch with http:// - should upgrade to https://
    let params = json!({
        "url": cfg.web_fetch.http_url.as_str(),
        "prompt": "Get the page title"
    });

    let result = tool.execute(params, &ctx).await;
    let output = result.expect("expected fetch to succeed when web_fetch.enabled=true");
    let url = output
        .metadata
        .get("url")
        .and_then(|v| v.as_str())
        .expect("expected url metadata on success");
    assert!(
        url.starts_with("https://"),
        "expected upgraded https url, got: {url}"
    );
}

#[tokio::test]
async fn test_web_fetch_valid_url() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // Use a stable, reliable endpoint
    let params = json!({
        "url": cfg.web_fetch.https_url.as_str(),
        "prompt": "Get the main heading"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("example.com"));
    assert!(output.output.contains("Prompt:"));
    assert_eq!(output.metadata.get("cached"), Some(&json!(false))); // First fetch
}

#[tokio::test]
async fn test_web_fetch_html_to_markdown() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": cfg.web_fetch.https_url.as_str(),
        "prompt": "Extract text content"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // The output should contain markdown-formatted content
    // example.com has simple HTML that converts to readable text
    assert!(output.output.contains("Content from"));
    assert!(output.output.len() > 100); // Should have substantial content
}

#[tokio::test]
async fn test_web_fetch_prompt_included() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let test_prompt = "Find all headings and summarize";

    let params = json!({
        "url": cfg.web_fetch.https_url.as_str(),
        "prompt": test_prompt
    });

    let result = tool.execute(params, &ctx).await;
    let output = result.expect("expected fetch to succeed when web_fetch.enabled=true");
    assert!(output.output.contains(test_prompt));
    assert!(output.output.contains("Prompt:"));
}

#[tokio::test]
async fn test_web_fetch_metadata() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": cfg.web_fetch.https_url.as_str(),
        "prompt": "test"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();

    // Verify metadata fields
    assert!(output.metadata.contains_key("url"));
    assert!(output.metadata.contains_key("content_length"));
    assert!(output.metadata.contains_key("cached"));

    let content_length = output.metadata.get("content_length").unwrap().as_u64().unwrap();
    assert!(content_length > 0);
}

#[tokio::test]
async fn test_web_fetch_cache_hit() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": cfg.web_fetch.https_url.as_str(),
        "prompt": "test"
    });

    // First fetch
    let result1 = tool.execute(params.clone(), &ctx).await;
    assert!(result1.is_ok());

    let output1 = result1.unwrap();
    assert_eq!(output1.metadata.get("cached"), Some(&json!(false)));

    // Second fetch (should be cached)
    let result2 = tool.execute(params, &ctx).await;
    assert!(result2.is_ok());

    let output2 = result2.unwrap();
    assert_eq!(output2.metadata.get("cached"), Some(&json!(true)));

    // Content should be the same
    assert_eq!(
        output1.metadata.get("content_length"),
        output2.metadata.get("content_length")
    );
}

#[tokio::test]
async fn test_web_fetch_cache_miss_after_first() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // First URL
    let params1 = json!({
        "url": cfg.web_fetch.https_url.as_str(),
        "prompt": "test"
    });

    let result1 = tool.execute(params1, &ctx).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap().metadata.get("cached"), Some(&json!(false)));

    // Different URL (should not be cached)
    let params2 = json!({
        "url": cfg.web_fetch.cache_miss_url.as_str(),
        "prompt": "test"
    });

    let result2 = tool.execute(params2, &ctx).await;
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap().metadata.get("cached"), Some(&json!(false)));
}

#[tokio::test]
async fn test_web_fetch_invalid_url() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": "not-a-valid-url",
        "prompt": "test"
    });

    let result = tool.execute(params, &ctx).await;
    // Should fail with network or parsing error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_web_fetch_network_error() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": cfg.web_fetch.invalid_domain_url.as_str(),
        "prompt": "test"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::Other(_) => {
            // Expected - network error
        }
        _ => panic!("Expected Other error for network failure"),
    }
}

#[tokio::test]
async fn test_web_fetch_redirect_same_host() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": cfg.web_fetch.redirect_same_host_url.as_str(),
        "prompt": "test"
    });

    let result = tool.execute(params, &ctx).await;
    // Same-host redirects should work fine
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_web_fetch_redirect_different_host() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": cfg.web_fetch.redirect_different_host_url.as_str(),
        "prompt": "test"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should detect the redirect
    assert!(output.output.contains("redirect") || output.metadata.contains_key("redirect_url"));
}

#[tokio::test]
async fn test_web_fetch_large_page() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": cfg.web_fetch.large_page_url.as_str(),
        "prompt": "Extract the content"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let content_length = output.metadata.get("content_length").unwrap().as_u64().unwrap();
    assert!(content_length > 500); // Should have substantial content
}

#[tokio::test]
async fn test_web_fetch_empty_response() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": cfg.web_fetch.text_page_url.as_str(),
        "prompt": "Get robots.txt content"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should still have some content (even if minimal)
    assert!(output.metadata.get("content_length").is_some());
}

#[tokio::test]
async fn test_web_fetch_concurrent_requests() {
    let cfg = common::test_config();
    if !cfg.web_fetch.enabled {
        return;
    }

    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn multiple concurrent fetches
    let urls = cfg.web_fetch.concurrent_urls.clone();
    assert!(
        !urls.is_empty(),
        "web_fetch.concurrent_urls must not be empty when enabled"
    );

    let mut handles = vec![];

    for url in urls {
        let tool_clone = WebFetchTool::new();
        let ctx_clone = ctx.clone();
        let url_clone = url.to_string();

        let handle = tokio::spawn(async move {
            let params = json!({
                "url": url_clone,
                "prompt": "test"
            });

            tool_clone.execute(params, &ctx_clone).await
        });

        handles.push(handle);
    }

    // Wait for all to complete
    let results = futures::future::join_all(handles).await;

    // With network access, all should succeed.
    let successes = results
        .into_iter()
        .map(|join_result| join_result.expect("task join failed"))
        .filter(|tool_result| tool_result.is_ok())
        .count();

    assert_eq!(
        successes,
        cfg.web_fetch.concurrent_urls.len(),
        "expected all concurrent fetches to succeed with network access"
    );
}

#[tokio::test]
async fn test_web_fetch_missing_params() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // Missing prompt parameter
    let params = json!({
        "url": "https://example.com"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(_) => {
            // Expected - missing required parameter
        }
        _ => panic!("Expected InvalidParams error"),
    }
}
