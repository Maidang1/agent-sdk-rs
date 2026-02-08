use std::time::Duration;

/// Configuration for various timeout settings
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Timeout for establishing a connection
    pub connect_timeout: Duration,
    /// Timeout for the entire request (including response)
    pub request_timeout: Duration,
    /// Optional timeout for streaming responses
    pub stream_timeout: Option<Duration>,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(120),
            stream_timeout: Some(Duration::from_secs(300)),
        }
    }
}

impl TimeoutConfig {
    /// Create a new timeout configuration with custom values
    pub fn new(
        connect_timeout: Duration,
        request_timeout: Duration,
        stream_timeout: Option<Duration>,
    ) -> Self {
        Self {
            connect_timeout,
            request_timeout,
            stream_timeout,
        }
    }

    /// Create a timeout configuration with shorter timeouts for quick operations
    pub fn fast() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            stream_timeout: Some(Duration::from_secs(60)),
        }
    }

    /// Create a timeout configuration with longer timeouts for slow operations
    pub fn slow() -> Self {
        Self {
            connect_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(300),
            stream_timeout: Some(Duration::from_secs(600)),
        }
    }
}
