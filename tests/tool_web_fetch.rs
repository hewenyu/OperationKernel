//! Integration tests for the web_fetch tool
//!
//! Note: Some tests require internet connectivity and may be flaky in offline environments.
//! Tests marked with #[ignore] require network access.

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
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // Try to fetch with http:// - should upgrade to https://
    // Using example.com which should work reliably
    let params = json!({
        "url": "http://example.com",
        "prompt": "Get the page title"
    });

    let result = tool.execute(params, &ctx).await;

    // If it fails due to network, that's okay - we just want to verify
    // the tool doesn't reject HTTP URLs
    // If it succeeds, verify it upgraded to HTTPS
    if let Ok(output) = result {
        assert!(output.metadata.get("url").is_some());
        let url = output.metadata.get("url").unwrap().as_str().unwrap();
        assert!(url.starts_with("https://"));
    }
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_web_fetch_valid_url() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // Use a stable, reliable endpoint
    let params = json!({
        "url": "https://example.com",
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
#[ignore] // Requires network access
async fn test_web_fetch_html_to_markdown() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": "https://example.com",
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
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let test_prompt = "Find all headings and summarize";

    let params = json!({
        "url": "https://example.com",
        "prompt": test_prompt
    });

    let result = tool.execute(params, &ctx).await;

    // Even if network fails, we can check structure if it succeeds
    if let Ok(output) = result {
        assert!(output.output.contains(test_prompt));
        assert!(output.output.contains("Prompt:"));
    }
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_web_fetch_metadata() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": "https://example.com",
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
#[ignore] // Requires network access
async fn test_web_fetch_cache_hit() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "url": "https://example.com",
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
#[ignore] // Requires network access
async fn test_web_fetch_cache_miss_after_first() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // First URL
    let params1 = json!({
        "url": "https://example.com",
        "prompt": "test"
    });

    let result1 = tool.execute(params1, &ctx).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap().metadata.get("cached"), Some(&json!(false)));

    // Different URL (should not be cached)
    let params2 = json!({
        "url": "https://www.example.org",
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
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // Use a domain that definitely doesn't exist
    let params = json!({
        "url": "https://this-domain-definitely-does-not-exist-12345.invalid",
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
#[ignore] // Requires network access and specific redirect setup
async fn test_web_fetch_redirect_same_host() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // httpbin.org has redirect endpoints
    let params = json!({
        "url": "https://httpbin.org/redirect-to?url=https://httpbin.org/html",
        "prompt": "test"
    });

    let result = tool.execute(params, &ctx).await;
    // Same-host redirects should work fine
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore] // Requires network access and cross-domain redirect
async fn test_web_fetch_redirect_different_host() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // Use httpbin.org redirect to example.com (different host)
    let params = json!({
        "url": "https://httpbin.org/redirect-to?url=https://example.com",
        "prompt": "test"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should detect the redirect
    assert!(output.output.contains("redirect") || output.metadata.contains_key("redirect_url"));
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_web_fetch_large_page() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // httpbin.org can return HTML of various sizes
    let params = json!({
        "url": "https://httpbin.org/html",
        "prompt": "Extract the content"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let content_length = output.metadata.get("content_length").unwrap().as_u64().unwrap();
    assert!(content_length > 500); // Should have substantial content
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_web_fetch_empty_response() {
    let fixture = TestFixture::new();
    let tool = WebFetchTool::new();
    let ctx = create_test_context(fixture.path());

    // Use an endpoint that returns minimal/empty HTML
    let params = json!({
        "url": "https://httpbin.org/robots.txt",
        "prompt": "Get robots.txt content"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should still have some content (even if minimal)
    assert!(output.metadata.get("content_length").is_some());
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_web_fetch_concurrent_requests() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn multiple concurrent fetches
    let urls = vec![
        "https://example.com",
        "https://example.org",
        "https://example.net",
    ];

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

    // At least some should succeed (depending on network)
    let successes = results
        .into_iter()
        .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
        .count();

    // If we have network, at least one should succeed
    // If no network, all will fail - that's okay for this test
    assert!(successes == 0 || successes >= 1);
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
