# LLM Tool Calling Design

## Overview

This document outlines the design for integrating LLM tool calling capabilities into the Agent SDK, leveraging the existing event-driven architecture and tool management system.

## Current Architecture Analysis

### Existing Components
- **Runtime**: Core execution environment with tool registration
- **EventBus**: Three-channel event system (Progress, Control, Monitor)
- **Tool Trait**: Standardized tool interface with async execution
- **ApprovalManager**: Tool execution control and policies
- **LLMClient**: OpenAI-compatible API abstraction

### Key Strengths
- Event-driven architecture enables real-time monitoring
- Tool approval workflow provides security controls
- Async tool execution supports concurrent operations
- Extensible LLMClient trait allows multiple LLM providers

## Design Goals

1. **Seamless Integration**: Tool calling should work naturally with existing Runtime
2. **Event Transparency**: All tool calls emit appropriate events
3. **Approval Workflow**: Respect existing approval policies
4. **Error Handling**: Robust error propagation and recovery
5. **Performance**: Minimize latency in tool call chains
6. **Extensibility**: Support different LLM tool calling formats

## Proposed Architecture

### 1. Tool Call Flow

```
LLM Response → Tool Call Parser → Approval Check → Tool Execution → Result Formatting → LLM Context
```

### 2. Core Components

#### ToolCallManager
- Parses LLM responses for tool calls
- Manages tool call execution lifecycle
- Handles parallel tool execution
- Formats results for LLM consumption

#### ToolCallParser
- Extracts tool calls from LLM responses
- Supports multiple formats (OpenAI, Anthropic, etc.)
- Validates tool call structure

#### ToolCallExecutor
- Executes approved tool calls
- Manages concurrent execution
- Handles timeouts and retries
- Emits progress events

### 3. Event Integration

#### New Event Types
```rust
// Progress Events
ToolCallRequested { tool_calls: Vec<ToolCall> }
ToolCallApproved { tool_call: ToolCall }
ToolCallRejected { tool_call: ToolCall, reason: String }
ToolCallStarted { tool_call: ToolCall }
ToolCallCompleted { tool_call: ToolCall, result: ToolResult }
ToolCallFailed { tool_call: ToolCall, error: String }

// Control Events
PauseToolExecution
ResumeToolExecution
CancelToolCall { call_id: String }

// Monitor Events
ToolCallMetrics { duration: Duration, success: bool }
```

### 4. Runtime Integration

#### Enhanced Runtime Options
```rust
pub struct RuntimeOptions {
    // Existing fields...
    
    // Tool calling configuration
    pub enable_tool_calling: bool,
    pub max_concurrent_tools: usize,
    pub tool_call_timeout: Duration,
    pub auto_format_results: bool,
    pub parallel_execution: bool,
}
```

#### Tool Call Lifecycle
1. **Detection**: Parse LLM response for tool calls
2. **Validation**: Verify tool exists and parameters are valid
3. **Approval**: Check approval policies
4. **Execution**: Run tool(s) with event emission
5. **Formatting**: Format results for LLM context
6. **Continuation**: Send results back to LLM

### 5. Approval Integration

#### Enhanced Approval Policies
- **ToolCallPolicy**: Specific policies for tool calling scenarios
- **BatchApproval**: Handle multiple tool calls efficiently
- **ConditionalApproval**: Approve based on context/parameters

#### Approval Flow
```
Tool Call → Policy Check → User Prompt (if needed) → Execution/Rejection
```

### 6. Error Handling Strategy

#### Error Categories
1. **Parse Errors**: Invalid tool call format
2. **Validation Errors**: Unknown tool or invalid parameters
3. **Approval Errors**: Rejected by approval policy
4. **Execution Errors**: Tool execution failures
5. **Timeout Errors**: Tool execution exceeded limits

#### Recovery Mechanisms
- Automatic retry for transient failures
- Fallback to alternative tools
- Graceful degradation without tool calling
- Error context preservation for LLM

### 7. Performance Considerations

#### Optimization Strategies
- **Parallel Execution**: Run independent tools concurrently
- **Result Caching**: Cache tool results for repeated calls
- **Lazy Loading**: Load tools on-demand
- **Connection Pooling**: Reuse HTTP connections for external tools

#### Monitoring
- Tool execution metrics
- Success/failure rates
- Performance bottlenecks
- Resource usage tracking

## Implementation Phases

### Phase 1: Core Infrastructure
- ToolCallManager implementation
- Basic parsing for OpenAI format
- Event integration
- Simple approval workflow

### Phase 2: Advanced Features
- Parallel tool execution
- Multiple LLM format support
- Enhanced error handling
- Performance optimizations

### Phase 3: Ecosystem Integration
- Multi-agent tool sharing
- Tool call scheduling
- Advanced approval policies
- Monitoring dashboard

## Configuration Examples

### Basic Setup
```rust
let runtime = Runtime::new(llm)
    .with_options(RuntimeOptions {
        enable_tool_calling: true,
        max_concurrent_tools: 3,
        tool_call_timeout: Duration::from_secs(30),
        ..Default::default()
    });
```

### With Approval
```rust
let approval = ApprovalManager::new()
    .with_tool_policy("file_operations", ApprovalPolicy::RequireApproval)
    .with_tool_policy("calculator", ApprovalPolicy::AutoApprove);

let runtime = Runtime::new(llm)
    .with_approval_manager(approval)
    .with_options(RuntimeOptions {
        enable_tool_calling: true,
        require_tool_approval: true,
        ..Default::default()
    });
```

### Event Monitoring
```rust
let event_bus = Arc::new(EventBus::new(1024));
let mut receiver = event_bus.subscribe();

tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        match event {
            AgentEvent::Progress(ProgressEvent::ToolCallStarted { tool_call }) => {
                println!("Executing: {}", tool_call.name);
            }
            AgentEvent::Progress(ProgressEvent::ToolCallCompleted { result, .. }) => {
                println!("Result: {}", result.content);
            }
            _ => {}
        }
    }
});
```

## Security Considerations

### Tool Execution Safety
- Sandboxed execution environment
- Resource limits (CPU, memory, network)
- Input validation and sanitization
- Output filtering

### Approval Workflow
- Mandatory approval for sensitive operations
- Audit logging for all tool calls
- User confirmation for destructive actions
- Policy inheritance in multi-agent scenarios

## Testing Strategy

### Unit Tests
- Tool call parsing accuracy
- Approval policy enforcement
- Error handling scenarios
- Event emission verification

### Integration Tests
- End-to-end tool calling flows
- Multi-agent tool sharing
- Performance under load
- Failure recovery scenarios

### Security Tests
- Input validation bypass attempts
- Resource exhaustion attacks
- Privilege escalation scenarios
- Audit trail verification

## Future Enhancements

### Advanced Features
- Tool composition and chaining
- Dynamic tool discovery
- Tool versioning and migration
- Cross-agent tool sharing protocols

### Ecosystem Integration
- Plugin marketplace
- Tool analytics and insights
- A/B testing for tool effectiveness
- Community tool contributions

## Conclusion

This design leverages the existing Agent SDK architecture while adding robust tool calling capabilities. The event-driven approach ensures transparency and control, while the approval system maintains security. The phased implementation allows for iterative development and testing.

The design prioritizes:
- **Reliability**: Robust error handling and recovery
- **Security**: Comprehensive approval and validation
- **Performance**: Efficient execution and monitoring
- **Extensibility**: Support for future enhancements

Next steps involve implementing Phase 1 components and validating the design through prototyping and testing.
