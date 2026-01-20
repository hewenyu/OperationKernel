//! Integration tests for the web_search tool
//!
//! These are real network tests and are configured via `tests/config.toml`.

mod common;

use common::TestFixture;
use ok::tool::{base::*, web_search::WebSearchTool};
use serde_json::json;
use std::sync::Arc;

fn create_test_context(working_dir: std::path::PathBuf) -> ToolContext {
    ToolContext::new(
        "test_session",
        "test_msg",
        "test_station",
        working_dir,
        Arc::new(ok::process::BackgroundShellManager::new()),
    )
}

fn configure_search_env() {
    let cfg = common::test_config();
    assert!(
        cfg.web_search.enabled,
        "web_search tests require web_search.enabled=true in tests/config.toml"
    );
    assert!(
        !cfg.web_search.provider.trim().is_empty(),
        "web_search.provider must be set in tests/config.toml"
    );

    std::env::set_var("SEARCH_PROVIDER", cfg.web_search.provider.trim());

    // The only supported provider today is brave.
    if cfg.web_search.provider.trim() == "brave" {
        assert!(
            !cfg.web_search.brave_api_key.trim().is_empty(),
            "web_search.brave_api_key must be set in tests/config.toml"
        );
        std::env::set_var("BRAVE_API_KEY", cfg.web_search.brave_api_key.trim());
    }
}

#[tokio::test]
async fn test_web_search_real_query_returns_results() {
    let cfg = common::test_config();
    if !cfg.web_search.enabled {
        return;
    }

    let _env_guard = common::env_lock();
    configure_search_env();

    let fixture = TestFixture::new();
    let tool = WebSearchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "query": cfg.web_search.query.as_str(),
        "allowed_domains": &cfg.web_search.allowed_domains,
        "blocked_domains": &cfg.web_search.blocked_domains,
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok(), "expected search to succeed when enabled and configured");

    let output = result.unwrap();
    let num_results = output
        .metadata
        .get("num_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    assert!(
        num_results >= cfg.web_search.min_results,
        "expected at least {} results, got {}",
        cfg.web_search.min_results,
        num_results
    );
    assert!(output.output.contains("Search:"));
}

#[tokio::test]
async fn test_web_search_cache_reuses_results_in_same_instance() {
    let cfg = common::test_config();
    if !cfg.web_search.enabled {
        return;
    }

    let _env_guard = common::env_lock();
    configure_search_env();

    let fixture = TestFixture::new();
    let tool = WebSearchTool::new();
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "query": cfg.web_search.query.as_str(),
        "allowed_domains": &cfg.web_search.allowed_domains,
        "blocked_domains": &cfg.web_search.blocked_domains,
    });

    let first = tool.execute(params.clone(), &ctx).await.unwrap();
    let second = tool.execute(params, &ctx).await.unwrap();

    assert_eq!(first.metadata.get("num_results"), second.metadata.get("num_results"));
    assert_eq!(first.output, second.output);
}
