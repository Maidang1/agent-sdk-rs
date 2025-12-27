mod open_router;

pub use open_router::OpenRouterProvider;

use std::future::Future;
use std::pin::Pin;

/// 消息角色
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// 聊天消息
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

/// 生成参数配置
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
}

/// Token 使用统计
#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 生成响应
#[derive(Debug, Clone)]
pub struct GenerateResponse {
    pub content: String,
    pub usage: Option<Usage>,
    pub model: String,
    pub finish_reason: Option<String>,
}

/// Provider 错误类型
#[derive(Debug)]
pub enum ProviderError {
    /// API 请求失败
    RequestFailed(String),
    /// 认证失败
    AuthenticationFailed,
    /// 速率限制
    RateLimited { retry_after: Option<u64> },
    /// 模型不可用
    ModelNotAvailable(String),
    /// 响应解析失败
    ParseError(String),
    /// 其他错误
    Other(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            Self::AuthenticationFailed => write!(f, "Authentication failed"),
            Self::RateLimited { retry_after } => {
                write!(f, "Rate limited")?;
                if let Some(secs) = retry_after {
                    write!(f, ", retry after {} seconds", secs)?;
                }
                Ok(())
            }
            Self::ModelNotAvailable(model) => write!(f, "Model not available: {}", model),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ProviderError {}

pub type Result<T> = std::result::Result<T, ProviderError>;

/// 大模型 Provider trait
/// 
/// 定义了与大语言模型交互的统一接口
pub trait LlmProvider: Send + Sync {
    /// 返回 provider 名称
    fn name(&self) -> &str;

    /// 返回当前使用的模型
    fn model(&self) -> &str;

    /// 生成文本响应
    fn generate(
        &self,
        messages: Vec<Message>,
        options: Option<GenerateOptions>,
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse>> + Send + '_>>;

    /// 流式生成（可选实现）
    fn generate_stream(
        &self,
        messages: Vec<Message>,
        options: Option<GenerateOptions>,
    ) -> Pin<Box<dyn Future<Output = Result<StreamResponse>> + Send + '_>> {
        Box::pin(async { Err(ProviderError::Other("Streaming not supported".into())) })
    }

    /// 检查 provider 是否可用
    fn health_check(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

/// 流式响应（简化版）
pub struct StreamResponse {
    pub receiver: tokio::sync::mpsc::Receiver<Result<String>>,
}
