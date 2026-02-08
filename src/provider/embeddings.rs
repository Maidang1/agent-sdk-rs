use std::future::Future;
use std::pin::Pin;
use super::{Result, ProviderError};

/// Request for creating embeddings
#[derive(Debug, Clone)]
pub struct EmbeddingRequest {
    /// Input texts to embed
    pub input: Vec<String>,
    /// Optional model to use (provider-specific)
    pub model: Option<String>,
    /// Optional encoding format
    pub encoding_format: Option<EncodingFormat>,
}

impl EmbeddingRequest {
    /// Create a new embedding request with a single input
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: vec![input.into()],
            model: None,
            encoding_format: None,
        }
    }

    /// Create a new embedding request with multiple inputs
    pub fn new_batch(inputs: Vec<String>) -> Self {
        Self {
            input: inputs,
            model: None,
            encoding_format: None,
        }
    }

    /// Set the model to use
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the encoding format
    pub fn with_encoding_format(mut self, format: EncodingFormat) -> Self {
        self.encoding_format = Some(format);
        self
    }
}

/// Encoding format for embeddings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingFormat {
    /// Return embeddings as floating point arrays
    Float,
    /// Return embeddings as base64-encoded strings
    Base64,
}

/// Response from creating embeddings
#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    /// The embedding vectors
    pub embeddings: Vec<Vec<f32>>,
    /// The model used
    pub model: String,
    /// Optional usage information
    pub usage: Option<EmbeddingUsage>,
}

impl EmbeddingResponse {
    /// Get the first embedding (convenience method for single input)
    pub fn first(&self) -> Option<&Vec<f32>> {
        self.embeddings.first()
    }

    /// Get the number of embeddings
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Check if there are no embeddings
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }
}

/// Usage information for embeddings
#[derive(Debug, Clone)]
pub struct EmbeddingUsage {
    /// Number of tokens in the input
    pub prompt_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
}

/// Trait for providers that support embeddings
pub trait EmbeddingProvider: Send + Sync {
    /// Create embeddings for the given input
    fn create_embeddings(
        &self,
        request: EmbeddingRequest,
    ) -> Pin<Box<dyn Future<Output = Result<EmbeddingResponse>> + Send + '_>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_request_builder() {
        let request = EmbeddingRequest::new("test input")
            .with_model("text-embedding-ada-002")
            .with_encoding_format(EncodingFormat::Float);

        assert_eq!(request.input.len(), 1);
        assert_eq!(request.input[0], "test input");
        assert_eq!(request.model, Some("text-embedding-ada-002".to_string()));
        assert_eq!(request.encoding_format, Some(EncodingFormat::Float));
    }

    #[test]
    fn test_embedding_request_batch() {
        let inputs = vec!["input1".to_string(), "input2".to_string()];
        let request = EmbeddingRequest::new_batch(inputs);

        assert_eq!(request.input.len(), 2);
    }

    #[test]
    fn test_embedding_response() {
        let response = EmbeddingResponse {
            embeddings: vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
            model: "test-model".to_string(),
            usage: Some(EmbeddingUsage {
                prompt_tokens: 10,
                total_tokens: 10,
            }),
        };

        assert_eq!(response.len(), 2);
        assert!(!response.is_empty());
        assert_eq!(response.first().unwrap(), &vec![0.1, 0.2, 0.3]);
    }
}
