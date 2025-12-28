# Agent SDK Tool Calling Design

## 现状分析

当前 Agent SDK 只实现了基础的 LLM Provider 层：

### 已实现组件
- **LlmProvider Trait**: 统一的 LLM 接口，支持同步和流式生成
- **OpenRouterProvider**: OpenRouter API 的具体实现
- **Message/Role**: 基础的消息和角色定义
- **Error Handling**: 完整的错误类型定义

### 缺失组件
- Runtime、EventBus、Tool Trait、ApprovalManager 等都未实现
- 需要从零开始设计和实现

## 设计目标

基于现有的 Provider 架构，设计一个轻量级但可扩展的工具调用系统：

1. **最小化实现**: 先实现核心功能，避免过度设计
2. **渐进式架构**: 支持后续扩展到事件驱动和多代理
3. **Provider 兼容**: 与现有 LlmProvider 无缝集成
4. **工具标准化**: 定义清晰的工具接口

## 核心架构设计

### 1. Tool System

#### Tool Trait
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    async fn execute(&self, params: &serde_json::Value) -> ToolResult;
}

pub struct ToolResult {
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
}
```

#### ToolRegistry
```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, tool: Box<dyn Tool>);
    pub fn get(&self, name: &str) -> Option<&dyn Tool>;
    pub fn list_tools(&self) -> Vec<ToolInfo>;
}
```

### 2. Agent Runtime

#### Agent
```rust
pub struct Agent<P: LlmProvider> {
    provider: P,
    tools: ToolRegistry,
    conversation: Vec<Message>,
    options: AgentOptions,
}

pub struct AgentOptions {
    pub system_prompt: Option<String>,
    pub max_iterations: usize,
    pub tool_choice: ToolChoice,
    pub generate_options: GenerateOptions,
}

pub enum ToolChoice {
    Auto,
    None,
    Required,
    Specific(String),
}
```

#### Core Methods
```rust
impl<P: LlmProvider> Agent<P> {
    pub fn new(provider: P) -> Self;
    pub fn with_options(self, options: AgentOptions) -> Self;
    pub fn register_tool(&mut self, tool: Box<dyn Tool>);
    
    pub async fn run(&mut self, input: &str) -> Result<String>;
    pub async fn run_stream(&mut self, input: &str) -> Result<StreamResponse>;
    
    async fn process_tool_calls(&mut self, content: &str) -> Result<Vec<ToolResult>>;
    fn extract_tool_calls(&self, content: &str) -> Vec<ToolCall>;
    fn format_tool_results(&self, results: &[ToolResult]) -> String;
}
```

### 3. Tool Call Processing

#### ToolCall Structure
```rust
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub parameters: serde_json::Value,
}

pub struct ToolCallParser;

impl ToolCallParser {
    pub fn extract_from_content(content: &str) -> Vec<ToolCall>;
    pub fn parse_json_format(content: &str) -> Vec<ToolCall>;
    pub fn parse_xml_format(content: &str) -> Vec<ToolCall>;
}
```

#### Tool Execution
```rust
pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
}

impl ToolExecutor {
    pub async fn execute_calls(&self, calls: Vec<ToolCall>) -> Vec<ToolResult>;
    pub async fn execute_single(&self, call: &ToolCall) -> ToolResult;
}
```

### 4. 实现策略

#### Phase 1: 基础工具调用
- 实现 Tool trait 和 ToolRegistry
- 实现基础的 Agent 结构
- 支持简单的工具调用解析和执行
- JSON 格式的工具调用协议

#### Phase 2: 增强功能
- 流式工具调用支持
- 错误处理和重试机制
- 工具调用验证
- 多种解析格式支持

#### Phase 3: 高级特性
- 工具调用审批机制
- 并发工具执行
- 工具调用缓存
- 性能监控

## 工具调用协议

### JSON 格式
```json
{
  "tool_calls": [
    {
      "id": "call_1",
      "name": "calculator",
      "parameters": {
        "a": 10,
        "b": 5,
        "operation": "add"
      }
    }
  ]
}
```

### XML 格式 (备选)
```xml
<tool_call id="call_1" name="calculator">
  <parameters>
    <a>10</a>
    <b>5</b>
    <operation>add</operation>
  </parameters>
</tool_call>
```

## 使用示例

### 基础使用
```rust
use agent_sdk::{Agent, OpenRouterProvider, Tool, ToolResult};

// 创建工具
struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Perform arithmetic operations" }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"},
                "operation": {"type": "string", "enum": ["add", "sub", "mul", "div"]}
            },
            "required": ["a", "b", "operation"]
        })
    }
    
    async fn execute(&self, params: &serde_json::Value) -> ToolResult {
        let a = params["a"].as_f64().unwrap_or(0.0);
        let b = params["b"].as_f64().unwrap_or(0.0);
        let op = params["operation"].as_str().unwrap_or("add");
        
        let result = match op {
            "add" => a + b,
            "sub" => a - b,
            "mul" => a * b,
            "div" => if b != 0.0 { a / b } else { return ToolResult::error("Division by zero") },
            _ => return ToolResult::error("Unknown operation"),
        };
        
        ToolResult::success(result.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = OpenRouterProvider::new(
        std::env::var("OPEN_ROUTER_API_KEY")?,
        "google/gemini-2.5-flash-lite-preview-09-2025"
    );
    
    let mut agent = Agent::new(provider)
        .with_options(AgentOptions {
            system_prompt: Some("You are a helpful assistant with access to tools.".into()),
            tool_choice: ToolChoice::Auto,
            ..Default::default()
        });
    
    agent.register_tool(Box::new(CalculatorTool));
    
    let response = agent.run("What is 15 * 23?").await?;
    println!("{}", response);
    
    Ok(())
}
```

### 流式使用
```rust
let mut stream = agent.run_stream("Calculate 100 / 4 and then add 25").await?;
while let Some(chunk) = stream.receiver.recv().await {
    match chunk {
        Ok(content) => print!("{}", content),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## 目录结构

```
src/
├── lib.rs              # 公共接口导出
├── provider/           # LLM Provider (已存在)
│   ├── mod.rs
│   └── open_router.rs
├── tool/               # 工具系统
│   ├── mod.rs
│   ├── registry.rs
│   ├── executor.rs
│   └── parser.rs
├── agent/              # Agent 运行时
│   ├── mod.rs
│   ├── agent.rs
│   └── options.rs
└── error.rs            # 统一错误处理
```

## 错误处理

```rust
#[derive(Debug)]
pub enum AgentError {
    Provider(ProviderError),
    ToolNotFound(String),
    ToolExecutionFailed(String),
    ParseError(String),
    InvalidParameters(String),
}

impl From<ProviderError> for AgentError {
    fn from(err: ProviderError) -> Self {
        AgentError::Provider(err)
    }
}
```

## 测试策略

### 单元测试
- Tool trait 实现测试
- ToolRegistry 功能测试
- ToolCallParser 解析测试
- Agent 基础功能测试

### 集成测试
- 端到端工具调用流程
- 错误处理场景
- 流式响应测试
- 多工具协作测试

## 扩展路径

### 事件系统 (Phase 4)
- 添加 EventBus 支持工具调用事件
- 工具执行进度监控
- 异步事件处理

### 多代理系统 (Phase 5)
- AgentPool 管理多个 Agent
- Agent 间工具共享
- 协作工具调用

### 审批系统 (Phase 6)
- ApprovalManager 集成
- 工具调用权限控制
- 用户确认机制

## 总结

这个设计基于现有的 Provider 架构，提供了一个轻量级但可扩展的工具调用系统。重点是：

1. **渐进式实现**: 从基础功能开始，逐步添加高级特性
2. **Provider 兼容**: 与现有 LlmProvider 完全兼容
3. **清晰接口**: Tool trait 提供标准化的工具接口
4. **错误处理**: 完整的错误类型和处理机制
5. **扩展性**: 为未来的事件系统和多代理支持预留接口

下一步是实现 Phase 1 的核心组件，验证设计的可行性。
