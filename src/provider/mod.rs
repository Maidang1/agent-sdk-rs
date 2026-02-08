mod anthropic;
mod open_router;
mod client;
mod retry;
mod rate_limit;
mod timeout;
mod middleware;
mod context;
mod cache;
mod embeddings;
mod batch;

#[allow(unused_imports)]
pub use anthropic::AnthropicProvider;
pub use open_router::OpenRouterProvider;
pub use client::{ProviderClient, ProviderClientBuilder};
pub use retry::{RetryConfig, RetryPolicy};
pub use rate_limit::{RateLimitConfig, RateLimiter, RateLimitGuard, RateLimitStats};
pub use timeout::TimeoutConfig;
pub use middleware::{
    Middleware, MiddlewareChain, RequestContext, ResponseContext,
    LoggingMiddleware, TokenCounterMiddleware, MetricsMiddleware,
};
pub use context::{ContextWindowConfig, ContextWindowManager, TruncationStrategy};
pub use cache::{CacheConfig, CacheKey, ResponseCache, CacheStats};
pub use embeddings::{
    EmbeddingProvider, EmbeddingRequest, EmbeddingResponse, EmbeddingUsage, EncodingFormat,
};
pub use batch::{
    BatchProvider, BatchRequest, BatchResponse, SingleRequest, SingleResponse,
    execute_batch_concurrent, execute_batch_sequential,
};

use std::future::Future;
use std::pin::Pin;

/// 消息角色
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Content block in a message (text or image)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentBlock {
    Text { text: String },
    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },
}

/// Source of an image
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ImageSource {
    Url { url: String },
    Base64 {
        media_type: String, // "image/jpeg", "image/png", etc.
        data: String,       // base64 encoded
    },
}

/// Level of detail for image processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Low,  // Faster, cheaper
    High, // More detailed
    Auto, // Let API decide
}

/// 聊天消息
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::user_text(content)
    }

    pub fn user_text(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
        }
    }

    pub fn user_with_image(text: impl Into<String>, image: ImageSource) -> Self {
        Self {
            role: Role::User,
            content: vec![
                ContentBlock::Text { text: text.into() },
                ContentBlock::Image {
                    source: image,
                    detail: None,
                },
            ],
        }
    }

    pub fn user_with_image_url(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self::user_with_image(text, ImageSource::Url { url: url.into() })
    }

    pub fn user_with_image_base64(
        text: impl Into<String>,
        media_type: impl Into<String>,
        data: impl Into<String>,
    ) -> Self {
        Self::user_with_image(
            text,
            ImageSource::Base64 {
                media_type: media_type.into(),
                data: data.into(),
            },
        )
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
        }
    }

    /// Get the text content from all text blocks
    pub fn content_as_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Check if this message contains any images
    pub fn has_images(&self) -> bool {
        self.content.iter().any(|block| matches!(block, ContentBlock::Image { .. }))
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
#[derive(Debug, Clone)]
pub enum ProviderError {
    /// API 请求失败
    RequestFailed(String),
    /// 认证失败
    AuthenticationFailed(String),
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
            Self::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
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
        _messages: Vec<Message>,
        _options: Option<GenerateOptions>,
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
