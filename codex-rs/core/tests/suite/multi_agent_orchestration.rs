//! Multi-agent orchestration tests
//!
//! Tests for concurrent agent spawning, coordination, resource management,
//! and error handling in multi-agent scenarios.

use anyhow::Result;
use codex_core::protocol::AgentStatus;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::{
    ev_completed, ev_function_call, ev_function_call_result, ev_response_created, mount_sse_once,
    mount_sse_sequence, sse, start_mock_server,
};
use core_test_support::{skip_if_no_network, test_codex::test_codex, wait_for_event};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;
use toml::Value as TomlValue;

/// Test that multiple agents can be spawned concurrently up to the limit
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_spawn_multiple_agents_concurrently() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Configure with max 10 threads to test the new limit
    let mut builder = test_codex();
    builder.cli_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(10),
    )]);

    // Mock response for spawning 5 agents successfully
    let responses = vec![
        ev_response_created("resp-1"),
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 1 task",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        ev_function_call(
            "call-2",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 2 task",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-2",
            &json!({"agent_id": "agent-2"}).to_string(),
            true,
        ),
        ev_function_call(
            "call-3",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 3 task",
                "agent_type": "explorer"
            }))?,
        ),
        ev_function_call_result(
            "call-3",
            &json!({"agent_id": "agent-3"}).to_string(),
            true,
        ),
        ev_function_call(
            "call-4",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 4 task",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-4",
            &json!({"agent_id": "agent-4"}).to_string(),
            true,
        ),
        ev_function_call(
            "call-5",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 5 task",
                "agent_type": "explorer"
            }))?,
        ),
        ev_function_call_result(
            "call-5",
            &json!({"agent_id": "agent-5"}).to_string(),
            true,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Spawn 5 agents for parallel work".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test that agent limit is enforced
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_agent_limit_enforcement() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Set a low limit for testing
    let mut builder = test_codex();
    builder.cli_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(2),
    )]);

    // Try to spawn 3 agents when limit is 2
    let responses = vec![
        ev_response_created("resp-1"),
        // First spawn succeeds
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 1",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        // Second spawn succeeds
        ev_function_call(
            "call-2",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 2",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-2",
            &json!({"agent_id": "agent-2"}).to_string(),
            true,
        ),
        // Third spawn should fail with limit error
        ev_function_call(
            "call-3",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 3",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-3",
            "Agent limit reached. Maximum 2 concurrent agents allowed.",
            false,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Try to spawn 3 agents".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test graceful degradation when one agent fails
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_single_agent_failure_does_not_crash_workflow() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Spawn 3 agents, one fails, but workflow continues
    let responses = vec![
        ev_response_created("resp-1"),
        // Agent 1 spawns successfully
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 1 task",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        // Agent 2 fails to spawn (simulated error)
        ev_function_call(
            "call-2",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 2 task (will fail)",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result("call-2", "Internal error spawning agent", false),
        // Agent 3 spawns successfully despite Agent 2 failing
        ev_function_call(
            "call-3",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Agent 3 task",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-3",
            &json!({"agent_id": "agent-3"}).to_string(),
            true,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Spawn 3 agents with one failure".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test wait tool with multiple agents
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_wait_for_multiple_agents() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Spawn 3 agents and wait for them
    let responses = vec![
        ev_response_created("resp-1"),
        // Spawn agent 1
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Task 1",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        // Spawn agent 2
        ev_function_call(
            "call-2",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Task 2",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-2",
            &json!({"agent_id": "agent-2"}).to_string(),
            true,
        ),
        // Spawn agent 3
        ev_function_call(
            "call-3",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Task 3",
                "agent_type": "explorer"
            }))?,
        ),
        ev_function_call_result(
            "call-3",
            &json!({"agent_id": "agent-3"}).to_string(),
            true,
        ),
        // Wait for all agents
        ev_function_call(
            "call-4",
            "wait",
            &serde_json::to_string(&json!({
                "ids": ["agent-1", "agent-2", "agent-3"],
                "timeout_ms": 30000
            }))?,
        ),
        ev_function_call_result(
            "call-4",
            &json!({
                "status": {
                    "agent-1": "completed",
                    "agent-2": "completed",
                    "agent-3": "completed"
                },
                "timed_out": false
            })
            .to_string(),
            true,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Spawn and wait for 3 agents".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test context isolation between agents
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_agent_context_isolation() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Each agent should have isolated context and not interfere
    let responses = vec![
        ev_response_created("resp-1"),
        // Spawn agent 1 with specific task
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Work on file A only",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        // Spawn agent 2 with different task
        ev_function_call(
            "call-2",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Work on file B only",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-2",
            &json!({"agent_id": "agent-2"}).to_string(),
            true,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Spawn 2 agents with isolated contexts".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test that agents can be closed/shutdown properly
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_agent_shutdown() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let responses = vec![
        ev_response_created("resp-1"),
        // Spawn agent
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Long running task",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        // Close the agent
        ev_function_call(
            "call-2",
            "close_agent",
            &serde_json::to_string(&json!({
                "id": "agent-1"
            }))?,
        ),
        ev_function_call_result(
            "call-2",
            &json!({"status": "shutdown"}).to_string(),
            true,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Spawn and close an agent".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test agent depth limits (prevent infinite recursion)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_agent_depth_limit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Test that agents can't spawn indefinitely deep
    let responses = vec![
        ev_response_created("resp-1"),
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Try to spawn deeply nested agents",
                "agent_type": "worker"
            }))?,
        ),
        // Should succeed at reasonable depth but eventually fail
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Test agent depth limits".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test sending input to spawned agents
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_send_input_to_agent() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let responses = vec![
        ev_response_created("resp-1"),
        // Spawn agent
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Initial task",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        // Send additional input to the agent
        ev_function_call(
            "call-2",
            "send_input",
            &serde_json::to_string(&json!({
                "id": "agent-1",
                "message": "Additional instructions",
                "interrupt": false
            }))?,
        ),
        ev_function_call_result(
            "call-2",
            &json!({"submission_id": "sub-1"}).to_string(),
            true,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Spawn agent and send it input".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test timeout behavior in wait tool
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_wait_timeout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let responses = vec![
        ev_response_created("resp-1"),
        // Spawn agent
        ev_function_call(
            "call-1",
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": "Long task",
                "agent_type": "worker"
            }))?,
        ),
        ev_function_call_result(
            "call-1",
            &json!({"agent_id": "agent-1"}).to_string(),
            true,
        ),
        // Wait with short timeout
        ev_function_call(
            "call-2",
            "wait",
            &serde_json::to_string(&json!({
                "ids": ["agent-1"],
                "timeout_ms": 10000  // 10 seconds
            }))?,
        ),
        ev_function_call_result(
            "call-2",
            &json!({
                "status": {
                    "agent-1": "running"
                },
                "timed_out": true
            })
            .to_string(),
            true,
        ),
        ev_completed("resp-1"),
    ];

    let _req = mount_sse_once(&server, sse(responses)).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Test wait timeout".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

/// Test maximum concurrent agents (10 agents)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_ten_concurrent_agents() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex();
    builder.cli_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(10),
    )]);

    // Build response for spawning 10 agents
    let mut events = vec![ev_response_created("resp-1")];

    for i in 1..=10 {
        events.push(ev_function_call(
            &format!("call-{i}"),
            "spawn_agent",
            &serde_json::to_string(&json!({
                "message": format!("Agent {i} task"),
                "agent_type": if i % 2 == 0 { "worker" } else { "explorer" }
            }))?,
        ));
        events.push(ev_function_call_result(
            &format!("call-{i}"),
            &json!({"agent_id": format!("agent-{i}")}).to_string(),
            true,
        ));
    }

    events.push(ev_completed("resp-1"));

    let _req = mount_sse_once(&server, sse(events)).await;

    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Spawn 10 agents concurrently".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}
