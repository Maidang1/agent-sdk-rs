use std::future::Future;
use std::pin::Pin;
use futures_util::stream::{self, StreamExt};
use super::{Message, GenerateOptions, GenerateResponse, LlmProvider, Result};

/// A single request in a batch
#[derive(Debug, Clone)]
pub struct SingleRequest {
    /// Unique identifier for this request
    pub id: String,
    /// Messages for this request
    pub messages: Vec<Message>,
    /// Optional generation options
    pub options: Option<GenerateOptions>,
}

impl SingleRequest {
    /// Create a new single request
    pub fn new(id: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            id: id.into(),
            messages,
            options: None,
        }
    }

    /// Create a new single request with options
    pub fn with_options(
        id: impl Into<String>,
        messages: Vec<Message>,
        options: GenerateOptions,
    ) -> Self {
        Self {
            id: id.into(),
            messages,
            options: Some(options),
        }
    }
}

/// A batch of requests to process
#[derive(Debug, Clone)]
pub struct BatchRequest {
    /// The requests to process
    pub requests: Vec<SingleRequest>,
    /// Maximum number of concurrent requests (None = unlimited)
    pub max_concurrent: Option<usize>,
}

impl BatchRequest {
    /// Create a new batch request
    pub fn new(requests: Vec<SingleRequest>) -> Self {
        Self {
            requests,
            max_concurrent: Some(5), // Default to 5 concurrent requests
        }
    }

    /// Set the maximum number of concurrent requests
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Allow unlimited concurrent requests
    pub fn unlimited_concurrent(mut self) -> Self {
        self.max_concurrent = None;
        self
    }

    /// Get the number of requests in this batch
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
}

/// A single response from a batch
#[derive(Debug, Clone)]
pub struct SingleResponse {
    /// The ID of the request this response corresponds to
    pub id: String,
    /// The result (success or error)
    pub result: Result<GenerateResponse>,
}

impl SingleResponse {
    /// Check if this response was successful
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    /// Check if this response was an error
    pub fn is_error(&self) -> bool {
        self.result.is_err()
    }
}

/// Response from a batch request
#[derive(Debug, Clone)]
pub struct BatchResponse {
    /// The responses for each request
    pub responses: Vec<SingleResponse>,
}

impl BatchResponse {
    /// Get the number of successful responses
    pub fn success_count(&self) -> usize {
        self.responses.iter().filter(|r| r.is_success()).count()
    }

    /// Get the number of failed responses
    pub fn error_count(&self) -> usize {
        self.responses.iter().filter(|r| r.is_error()).count()
    }

    /// Get all successful responses
    pub fn successes(&self) -> Vec<&SingleResponse> {
        self.responses.iter().filter(|r| r.is_success()).collect()
    }

    /// Get all failed responses
    pub fn errors(&self) -> Vec<&SingleResponse> {
        self.responses.iter().filter(|r| r.is_error()).collect()
    }

    /// Check if all requests succeeded
    pub fn all_succeeded(&self) -> bool {
        self.responses.iter().all(|r| r.is_success())
    }

    /// Check if any requests failed
    pub fn any_failed(&self) -> bool {
        self.responses.iter().any(|r| r.is_error())
    }
}

/// Trait for providers that support batch requests
pub trait BatchProvider: Send + Sync {
    /// Process a batch of requests
    fn generate_batch(
        &self,
        batch: BatchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<BatchResponse>> + Send + '_>>;
}

/// Execute a batch of requests concurrently using any LlmProvider
pub async fn execute_batch_concurrent<P: LlmProvider>(
    provider: &P,
    batch: BatchRequest,
) -> Result<BatchResponse> {
    let max_concurrent = batch.max_concurrent.unwrap_or(usize::MAX);

    let responses = stream::iter(batch.requests)
        .map(|req| async move {
            let result = provider.generate(req.messages, req.options).await;
            SingleResponse { id: req.id, result }
        })
        .buffer_unordered(max_concurrent)
        .collect::<Vec<_>>()
        .await;

    Ok(BatchResponse { responses })
}

/// Execute a batch of requests sequentially using any LlmProvider
pub async fn execute_batch_sequential<P: LlmProvider>(
    provider: &P,
    batch: BatchRequest,
) -> Result<BatchResponse> {
    let mut responses = Vec::new();

    for req in batch.requests {
        let result = provider.generate(req.messages, req.options).await;
        responses.push(SingleResponse { id: req.id, result });
    }

    Ok(BatchResponse { responses })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_request_builder() {
        let requests = vec![
            SingleRequest::new("1", vec![]),
            SingleRequest::new("2", vec![]),
        ];

        let batch = BatchRequest::new(requests)
            .with_max_concurrent(10);

        assert_eq!(batch.len(), 2);
        assert_eq!(batch.max_concurrent, Some(10));
    }

    #[test]
    fn test_batch_response_stats() {
        use crate::provider::{ProviderError, Usage};

        let responses = vec![
            SingleResponse {
                id: "1".to_string(),
                result: Ok(GenerateResponse {
                    content: "success".to_string(),
                    usage: Some(Usage {
                        prompt_tokens: 10,
                        completion_tokens: 20,
                        total_tokens: 30,
                    }),
                    model: "test".to_string(),
                    finish_reason: None,
                }),
            },
            SingleResponse {
                id: "2".to_string(),
                result: Err(ProviderError::RequestFailed("error".to_string())),
            },
        ];

        let batch_response = BatchResponse { responses };

        assert_eq!(batch_response.success_count(), 1);
        assert_eq!(batch_response.error_count(), 1);
        assert!(!batch_response.all_succeeded());
        assert!(batch_response.any_failed());
    }

    #[test]
    fn test_single_request() {
        let request = SingleRequest::new("test-id", vec![]);
        assert_eq!(request.id, "test-id");
        assert!(request.options.is_none());
    }

    #[test]
    fn test_single_response() {
        use crate::provider::{ProviderError, Usage};

        let success = SingleResponse {
            id: "1".to_string(),
            result: Ok(GenerateResponse {
                content: "test".to_string(),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
                model: "test".to_string(),
                finish_reason: None,
            }),
        };

        let error = SingleResponse {
            id: "2".to_string(),
            result: Err(ProviderError::RequestFailed("error".to_string())),
        };

        assert!(success.is_success());
        assert!(!success.is_error());
        assert!(!error.is_success());
        assert!(error.is_error());
    }
}
