# Tool Parameter Validation

Agent SDK 现在支持自动参数校验功能，确保工具在执行前接收到正确格式的参数。

## 功能特性

- ✅ **自动校验**: 在工具执行前自动验证参数
- ✅ **JSON Schema 支持**: 基于工具的 `parameters_schema()` 进行校验
- ✅ **类型检查**: 验证参数类型（string, number, boolean, array, object）
- ✅ **必需字段**: 检查必需参数是否存在
- ✅ **枚举值**: 验证参数值是否在允许的枚举范围内
- ✅ **错误提示**: 提供详细的校验错误信息

## 校验规则

### 1. 必需字段检查
```json
{
  "required": ["name", "age"]
}
```
如果缺少必需字段，返回错误：`Missing required parameter: age`

### 2. 类型检查
```json
{
  "properties": {
    "age": {"type": "number"}
  }
}
```
如果类型不匹配，返回错误：`Parameter 'age' must be of type 'number', got 'string'`

### 3. 枚举值检查
```json
{
  "properties": {
    "status": {"type": "string", "enum": ["active", "inactive"]}
  }
}
```
如果值不在枚举范围内，返回错误：`Parameter 'status' must be one of: ["active", "inactive"]`

## 使用示例

### 定义带校验的工具

```rust
use agent_sdk::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

struct ValidatedTool;

#[async_trait]
impl Tool for ValidatedTool {
    fn name(&self) -> &str {
        "validated_tool"
    }

    fn description(&self) -> &str {
        "A tool with parameter validation"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"},
                "status": {"type": "string", "enum": ["active", "inactive"]}
            },
            "required": ["name", "age"]
        })
    }

    async fn execute(&self, params: &Value) -> ToolResult {
        // 参数已经过校验，可以安全使用 unwrap()
        let name = params["name"].as_str().unwrap();
        let age = params["age"].as_f64().unwrap();
        
        ToolResult::success(format!("Hello {}, age {}", name, age))
    }
}
```

### 自定义校验逻辑

如果需要更复杂的校验逻辑，可以重写 `validate_parameters` 方法：

```rust
impl Tool for CustomValidatedTool {
    // ... 其他方法

    fn validate_parameters(&self, params: &Value) -> Result<(), String> {
        // 先执行默认校验
        self.default_validate_parameters(params)?;
        
        // 添加自定义校验逻辑
        if let Some(age) = params["age"].as_f64() {
            if age < 0.0 || age > 150.0 {
                return Err("Age must be between 0 and 150".to_string());
            }
        }
        
        Ok(())
    }
}
```

## 校验流程

1. **工具调用**: LLM 生成工具调用请求
2. **参数解析**: 从 LLM 响应中提取工具调用参数
3. **参数校验**: 调用 `validate_parameters()` 进行校验
4. **校验失败**: 如果校验失败，返回错误信息给 LLM，不执行工具
5. **校验成功**: 如果校验通过，执行 `execute()` 方法
6. **返回结果**: 将执行结果返回给 LLM

## 错误处理

当参数校验失败时，系统会：

1. 生成详细的错误信息
2. 将错误信息包装为 `ToolResult::error()`
3. 将错误信息返回给 LLM，让 LLM 了解问题并可能重新尝试

## 性能考虑

- 校验在工具执行前进行，避免无效执行
- 校验逻辑轻量级，对性能影响最小
- 错误信息清晰，帮助 LLM 快速纠正参数

## 测试

运行参数校验测试：

```bash
# 直接校验测试
cargo run --example direct_validation

# 完整工具调用测试
cargo run --example validation_test
```

## 最佳实践

1. **明确的 Schema**: 提供清晰详细的参数 schema
2. **合理的枚举**: 为字符串参数提供枚举值限制
3. **必需字段**: 明确标记必需的参数
4. **错误信息**: 提供有意义的自定义校验错误信息
5. **安全执行**: 在 `execute()` 中可以安全使用 `unwrap()`，因为参数已校验
