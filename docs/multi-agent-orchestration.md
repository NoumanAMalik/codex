# Multi-Agent Orchestration in Codex

Codex includes a comprehensive multi-agent orchestration system that enables spawning and coordinating multiple agent threads in parallel. This capability allows for efficient task decomposition, parallel execution, and faster feature development.

## Overview

The multi-agent orchestration system provides:

- **Concurrent agent spawning** - Up to 10 parallel agent threads (configurable)
- **Inter-agent communication** - Send messages and coordinate between agents
- **Status monitoring** - Real-time tracking of agent states
- **Context isolation** - Each agent has its own isolated context and state
- **Resource management** - Guards prevent resource exhaustion
- **Graceful error handling** - Single agent failures don't crash the workflow
- **Role-based agents** - Explorer (fast research), Worker (execution), Orchestrator (coordination)

## Architecture

```
ThreadManagerState
  ├─ AgentControl (per user session)
  │   ├─ spawn_agent() - Create new agent thread
  │   ├─ send_prompt() - Send message to agent
  │   ├─ shutdown_agent() - Gracefully shutdown agent
  │   ├─ get_status() - Get current agent status
  │   └─ subscribe_status() - Subscribe to status updates
  ├─ Guards (resource limits)
  │   └─ reserve_spawn_slot() - Enforce max_threads limit
  └─ Multiple CodexThread instances (isolated contexts)
```

## Configuration

Configure the maximum number of concurrent agents in `config.toml`:

```toml
[agents]
max_threads = 10  # Default: 10, adjust based on system resources
```

## Usage

### Spawning Agents

Codex provides collaboration tools accessible to the model for spawning and managing agents:

#### `spawn_agent` Tool
```json
{
  "message": "Implement the new authentication feature",
  "agent_type": "worker"  // Optional: "worker", "explorer", or "orchestrator"
}
```

Returns:
```json
{
  "agent_id": "thread-abc123"
}
```

#### Agent Types

- **`explorer`** - Fast agents optimized for code exploration and information gathering
  - Uses faster model configurations
  - Lower reasoning effort for speed
  - Ideal for: finding code patterns, understanding structure, locating symbols

- **`worker`** - General-purpose execution agents
  - Standard configuration
  - Suitable for: implementing features, fixing bugs, running tests

- **`orchestrator`** - Coordination-focused agents (experimental)
  - Delegates to other agents
  - Manages complex multi-step workflows

### Inter-Agent Communication

#### `send_input` Tool
Send additional instructions to a running agent:

```json
{
  "id": "thread-abc123",
  "message": "Also add unit tests for the authentication flow",
  "interrupt": false  // Set to true to interrupt current task
}
```

#### `wait` Tool
Wait for agents to complete (or timeout):

```json
{
  "ids": ["thread-abc123", "thread-def456"],
  "timeout_ms": 30000  // 30 seconds (10s-300s range)
}
```

Returns status of all agents:
```json
{
  "status": {
    "thread-abc123": "completed",
    "thread-def456": "running"
  },
  "timed_out": false
}
```

#### `close_agent` Tool
Shutdown an agent gracefully:

```json
{
  "id": "thread-abc123"
}
```

### Agent Status

Agents can be in the following states:

- **`pending_init`** - Agent created, initializing
- **`running`** - Agent actively executing
- **`completed`** - Agent finished successfully
- **`errored`** - Agent encountered an error
- **`shutdown`** - Agent was explicitly shut down
- **`not_found`** - Agent ID doesn't exist

## Examples

### Parallel Feature Development

```
User: "Implement login, signup, and password reset features"

Agent spawns:
  1. Worker agent for login feature
  2. Worker agent for signup feature
  3. Worker agent for password reset feature
  
Wait for all agents to complete, then merge results.
```

### Research + Implementation

```
User: "Add caching to the database layer"

Agent spawns:
  1. Explorer agent to research existing caching patterns
  2. Wait for research results
  3. Worker agent to implement based on findings
```

### Independent File Changes

```
User: "Update all configuration files with new settings"

Agent spawns:
  - Worker for config.yaml
  - Worker for settings.json
  - Worker for .env.example
  
All agents work in parallel without conflicts.
```

## Resource Management

### Limits and Guards

The orchestration system enforces limits to prevent resource exhaustion:

- **Max concurrent threads**: Configurable (default: 10)
- **Depth limits**: Prevents infinite agent spawning
- **Timeout enforcement**: Wait operations have configurable timeouts (10s-300s)
- **Memory isolation**: Each agent has its own context window

### Best Practices

1. **Use appropriate agent types**
   - Explorer for fast research and code navigation
   - Worker for implementation tasks
   - Keep agent tasks focused and well-scoped

2. **Set reasonable timeouts**
   - Minimum 10 seconds to prevent busy-polling
   - Default 30 seconds for most tasks
   - Up to 300 seconds for long-running operations

3. **Monitor agent status**
   - Use the wait tool to track progress
   - Handle agent failures gracefully
   - Close agents when no longer needed

4. **Avoid deep nesting**
   - Agent depth is limited to prevent recursion
   - Orchestrator should manage agents, not agents spawning agents

## Error Handling

The system provides graceful degradation:

- **Single agent failures** - Other agents continue execution
- **Resource limits** - Clear error messages when limits exceeded
- **Timeout handling** - Wait operations return partial results on timeout
- **Cleanup on error** - Resources are released when agents fail

## Performance

Multi-agent orchestration provides significant speedup for parallelizable tasks:

- **Independent tasks**: Near-linear speedup with agent count
- **Research + implementation**: 2-3x faster than sequential
- **Multiple file changes**: Scales with number of agents

Typical scenarios:
- 3 parallel agents: ~3x speedup for independent tasks
- 5 parallel agents: ~5x speedup for file-scoped changes
- 10 parallel agents: Best for large refactoring across many files

## Monitoring and Visualization

### TUI Display

The Codex TUI displays real-time collaboration events:

- **Agent spawned** - Shows agent ID, status, and initial prompt
- **Input sent** - Displays receiver, prompt, and status
- **Waiting for agents** - Lists agents being waited on
- **Wait complete** - Summary of final statuses (completed, running, errored)
- **Agent closed** - Confirms shutdown with final status

Status indicators:
- 🟦 **Running** - Agent actively working (cyan)
- 🟩 **Completed** - Agent finished successfully (green)  
- 🟥 **Errored** - Agent encountered an error (red)
- ⚫ **Pending/Shutdown** - Agent initializing or shut down (dimmed)

### Programmatic Access

For applications integrating Codex:

```rust
use codex_core::agent::AgentControl;

// Subscribe to status updates
let mut status_rx = agent_control
    .subscribe_status(agent_id)
    .await?;

// Receive status changes
while status_rx.changed().await.is_ok() {
    let status = status_rx.borrow().clone();
    println!("Agent status: {:?}", status);
}
```

## Testing

Comprehensive tests validate the orchestration system:

- **Concurrent spawning** - Multiple agents spawn successfully
- **Limit enforcement** - Respects max_threads configuration
- **Failure isolation** - Single agent failures don't crash workflow
- **Context isolation** - Agents don't interfere with each other
- **Communication** - send_input, wait, and close_agent work correctly
- **Timeout handling** - Wait operations timeout appropriately
- **Load testing** - 10 concurrent agents perform reliably

Run tests:
```bash
cd codex-rs
cargo test -p codex-core multi_agent_orchestration
```

## Troubleshooting

### "Agent limit reached" error

Increase the limit in config.toml:
```toml
[agents]
max_threads = 15
```

### Agents timing out

Increase the timeout in wait calls, or check if agents are stuck.

### Performance degradation

- Reduce number of concurrent agents
- Ensure agents have focused, well-scoped tasks
- Check system resources (CPU, memory)

### Agent not found

Agent may have shut down or never spawned successfully. Check for spawn errors.

## Implementation Files

Core implementation:
- `codex-rs/core/src/agent/control.rs` - Agent lifecycle management
- `codex-rs/core/src/tools/handlers/collab.rs` - Collaboration tools
- `codex-rs/core/src/agent/guards.rs` - Resource limits
- `codex-rs/core/src/tools/parallel.rs` - Parallel execution

TUI visualization:
- `codex-rs/tui/src/collab.rs` - Event rendering

Tests:
- `codex-rs/core/tests/suite/multi_agent_orchestration.rs` - Integration tests
- `codex-rs/core/src/agent/control.rs` - Unit tests

## Future Enhancements

Planned improvements:
- Enhanced progress tracking with estimated completion times
- Dependency graph visualization
- Interactive agent management in TUI
- Performance metrics and profiling
- Automatic agent scaling based on workload
