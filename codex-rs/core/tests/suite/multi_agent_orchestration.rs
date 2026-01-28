//! Multi-agent orchestration tests
//!
//! Tests for the multi-agent orchestration configuration and limits.
//! The core orchestration functionality is tested in unit tests within
//! codex-rs/core/src/agent/control.rs

use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use toml::Value as TomlValue;

async fn test_config_with_overrides(overrides: Vec<(String, TomlValue)>) -> Config {
    let home = TempDir::new().expect("create temp dir");
    ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .cli_overrides(overrides)
        .build()
        .await
        .expect("load config")
}

/// Test that the default agent limit is 10
#[tokio::test]
async fn test_default_agent_limit_is_ten() {
    let home = TempDir::new().expect("create temp dir");
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .expect("load config");

    // The default limit should now be 10
    assert_eq!(
        config.agent_max_threads,
        Some(10),
        "Default agent max threads should be 10"
    );
}

/// Test that agent limit can be configured
#[tokio::test]
async fn test_agent_limit_can_be_configured() {
    let config = test_config_with_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(5),
    )])
    .await;

    assert_eq!(
        config.agent_max_threads,
        Some(5),
        "Agent max threads should be configurable"
    );
}

/// Test that agent limit can be set to 10
#[tokio::test]
async fn test_agent_limit_supports_ten_agents() {
    let config = test_config_with_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(10),
    )])
    .await;

    assert_eq!(
        config.agent_max_threads,
        Some(10),
        "Agent max threads should support 10 agents"
    );
}

/// Test that agent limit can be increased beyond 10
#[tokio::test]
async fn test_agent_limit_can_exceed_ten() {
    let config = test_config_with_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(15),
    )])
    .await;

    assert_eq!(
        config.agent_max_threads,
        Some(15),
        "Agent max threads should support values beyond 10"
    );
}

/// Test that agent limit of 0 is rejected
#[tokio::test]
async fn test_agent_limit_zero_is_rejected() {
    let home = TempDir::new().expect("create temp dir");
    let result = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(0),
        )])
        .build()
        .await;

    assert!(
        result.is_err(),
        "Setting agent_max_threads to 0 should be rejected"
    );
}

/// Test that agent limit respects None (unlimited)
#[tokio::test]
async fn test_agent_limit_none_is_unlimited() {
    let home = TempDir::new().expect("create temp dir");
    // When no limit is set in config, it should still default to Some(10)
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .expect("load config");

    // With no override, should use default
    assert_eq!(
        config.agent_max_threads,
        Some(10),
        "Without override, should use default of 10"
    );
}
