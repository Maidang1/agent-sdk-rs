# Event Subscription System

Agent SDK 现在支持完整的事件订阅系统，允许在大模型调用的关键节点触发相关的 hooks。

## 功能特性

- ✅ **事件总线**: 基于 tokio broadcast channel 的高性能事件系统
- ✅ **关键节点监控**: 覆盖对话和工具调用的所有重要阶段
- ✅ **Hook 系统**: 支持自定义和预定义的事件处理器
- ✅ **异步处理**: 完全异步的事件处理，不阻塞主流程
- ✅ **多订阅者**: 支持多个监听器同时订阅事件

## 事件类型

### 对话生命周期事件
- `ConversationStarted` - 对话开始
- `ConversationCompleted` - 对话成功完成
- `ConversationFailed` - 对话失败

### LLM 交互事件
- `LlmRequestSent` - LLM 请求发送
- `LlmResponseReceived` - LLM 响应接收

### 工具调用事件
- `ToolCallsDetected` - 检测到工具调用
- `ToolCallStarted` - 工具调用开始
- `ToolCallCompleted` - 工具调用成功完成
- `ToolCallFailed` - 工具调用失败

## 基础使用

### 1. 创建事件总线和代理

```rust
use agent_sdk::{Agent, EventBus, AgentEvent};
use std::sync::Arc;

// 创建事件总线
let event_bus = Arc::new(EventBus::new(100));

// 创建带事件总线的代理
let mut agent = Agent::new(provider)
    .with_event_bus(event_bus.clone());
```

### 2. 订阅事件

```rust
// 获取事件接收器
let mut receiver = event_bus.subscribe();

// 启动事件监听器
tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        match event {
            AgentEvent::ConversationStarted { input } => {
                println!("对话开始: {}", input);
            }
            AgentEvent::ToolCallStarted { call } => {
                println!("工具调用开始: {}", call.name);
            }
            AgentEvent::ToolCallCompleted { call, result } => {
                println!("工具调用完成: {} -> {}", call.name, result.content);
            }
            _ => {}
        }
    }
});
```

## Hook 系统

### 预定义 Hooks

```rust
use agent_sdk::HookManager;

let mut hook_manager = HookManager::new(event_bus.clone());

// 日志记录 Hook
hook_manager.add_hook(HookManager::logging_hook());

// 指标统计 Hook
hook_manager.add_hook(HookManager::metrics_hook());

// 错误跟踪 Hook
hook_manager.add_hook(HookManager::error_tracking_hook());

// 启动监听
hook_manager.start_monitoring().await;
```

### 自定义 Hooks

```rust
// 添加自定义 Hook
hook_manager.add_hook(Arc::new(|event| {
    match event {
        AgentEvent::ToolCallStarted { call } => {
            // 记录工具调用开始时间
            println!("开始执行工具: {}", call.name);
        }
        AgentEvent::LlmResponseReceived { content, model } => {
            // 分析 LLM 响应
            if content.contains("error") {
                println!("检测到可能的错误响应");
            }
        }
        _ => {}
    }
    true // 返回 true 继续处理其他 hooks
}));
```

## 实际应用场景

### 1. 性能监控

```rust
use std::time::Instant;
use std::sync::Mutex;

static START_TIMES: Mutex<std::collections::HashMap<String, Instant>> = Mutex::new(std::collections::HashMap::new());

let performance_hook = Arc::new(|event| {
    match event {
        AgentEvent::ToolCallStarted { call } => {
            let mut times = START_TIMES.lock().unwrap();
            times.insert(call.id.clone(), Instant::now());
        }
        AgentEvent::ToolCallCompleted { call, .. } => {
            let mut times = START_TIMES.lock().unwrap();
            if let Some(start_time) = times.remove(&call.id) {
                let duration = start_time.elapsed();
                println!("工具 {} 执行耗时: {:?}", call.name, duration);
            }
        }
        _ => {}
    }
    true
});
```

### 2. 错误报告

```rust
let error_reporting_hook = Arc::new(|event| {
    match event {
        AgentEvent::ToolCallFailed { call, error } => {
            // 发送错误报告到监控系统
            eprintln!("工具调用失败: {} - {}", call.name, error);
            // send_to_monitoring_system(&call, &error);
        }
        AgentEvent::ConversationFailed { error } => {
            eprintln!("对话失败: {}", error);
            // send_alert(&error);
        }
        _ => {}
    }
    true
});
```

### 3. 审计日志

```rust
let audit_hook = Arc::new(|event| {
    match event {
        AgentEvent::ConversationStarted { input } => {
            // 记录用户输入
            log::info!("用户输入: {}", input);
        }
        AgentEvent::ToolCallStarted { call } => {
            // 记录工具调用
            log::info!("工具调用: {} 参数: {}", call.name, call.parameters);
        }
        AgentEvent::ConversationCompleted { response } => {
            // 记录最终响应
            log::info!("对话完成: {}", response);
        }
        _ => {}
    }
    true
});
```

## 事件流程图

```
用户输入
    ↓
ConversationStarted
    ↓
LlmRequestSent
    ↓
LlmResponseReceived
    ↓
ToolCallsDetected (如果有工具调用)
    ↓
ToolCallStarted (每个工具调用)
    ↓
ToolCallCompleted/Failed
    ↓
(重复 LLM 交互直到完成)
    ↓
ConversationCompleted/Failed
```

## 最佳实践

### 1. 事件处理性能
- Hook 函数应该快速执行，避免阻塞
- 对于耗时操作，在 Hook 中启动新的异步任务

### 2. 错误处理
- Hook 函数中的错误不应该影响主流程
- 使用 `Result` 类型处理可能的错误

### 3. 资源管理
- 合理设置事件总线容量
- 及时清理不再需要的事件监听器

### 4. 调试和监控
- 使用日志记录重要事件
- 实现指标收集用于性能分析

## 示例程序

```bash
# 基础事件监听
cargo run --example simple_events

# 完整事件监控
cargo run --example event_monitoring

# Hook 系统演示
cargo run --example hook_system
```

## 扩展性

事件系统设计为可扩展的：

1. **新事件类型**: 可以轻松添加新的事件类型
2. **自定义 Hook**: 支持任意复杂的事件处理逻辑
3. **多级监听**: 支持事件的层级处理和过滤
4. **持久化**: 可以将事件持久化到数据库或文件

这个事件系统为 Agent SDK 提供了强大的可观测性和扩展性，使得开发者可以深入了解和控制 AI 代理的行为。
