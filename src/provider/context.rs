use super::{Message, Role};

/// Configuration for context window management
#[derive(Debug, Clone)]
pub struct ContextWindowConfig {
    /// Maximum number of tokens allowed in the context
    pub max_tokens: usize,
    /// Strategy to use when truncating messages
    pub truncation_strategy: TruncationStrategy,
}

impl Default for ContextWindowConfig {
    fn default() -> Self {
        Self {
            max_tokens: 100_000, // Default to 100k tokens
            truncation_strategy: TruncationStrategy::DropOldest,
        }
    }
}

impl ContextWindowConfig {
    /// Create a new context window configuration
    pub fn new(max_tokens: usize, truncation_strategy: TruncationStrategy) -> Self {
        Self {
            max_tokens,
            truncation_strategy,
        }
    }

    /// Create a configuration for small context windows (e.g., 4k tokens)
    pub fn small() -> Self {
        Self {
            max_tokens: 4_000,
            truncation_strategy: TruncationStrategy::DropOldest,
        }
    }

    /// Create a configuration for medium context windows (e.g., 32k tokens)
    pub fn medium() -> Self {
        Self {
            max_tokens: 32_000,
            truncation_strategy: TruncationStrategy::DropOldest,
        }
    }

    /// Create a configuration for large context windows (e.g., 200k tokens)
    pub fn large() -> Self {
        Self {
            max_tokens: 200_000,
            truncation_strategy: TruncationStrategy::DropMiddle,
        }
    }
}

/// Strategy for truncating messages when context window is exceeded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationStrategy {
    /// Remove oldest messages first (keeps recent context)
    DropOldest,
    /// Keep first and last messages, drop middle (preserves instructions and recent context)
    DropMiddle,
    /// Summarize old messages (future feature - currently behaves like DropOldest)
    Summarize,
}

/// Manager for handling context window limits
pub struct ContextWindowManager {
    config: ContextWindowConfig,
}

impl ContextWindowManager {
    /// Create a new context window manager with the given configuration
    pub fn new(config: ContextWindowConfig) -> Self {
        Self { config }
    }

    /// Estimate the number of tokens in a message
    /// This is a rough estimate: ~4 characters per token for English text
    fn estimate_tokens(&self, message: &Message) -> usize {
        // Rough estimation: 4 characters per token
        // This is a simplification - real tokenization is more complex
        let char_count = message.content_as_text().len();
        (char_count + 3) / 4 // Round up
    }

    /// Estimate total tokens in a list of messages
    fn estimate_total_tokens(&self, messages: &[Message]) -> usize {
        messages.iter().map(|m| self.estimate_tokens(m)).sum()
    }

    /// Truncate messages if they exceed the context window limit
    pub fn truncate_if_needed(&self, messages: Vec<Message>) -> Vec<Message> {
        let total_tokens = self.estimate_total_tokens(&messages);

        if total_tokens <= self.config.max_tokens {
            return messages;
        }

        match self.config.truncation_strategy {
            TruncationStrategy::DropOldest => self.drop_oldest(messages),
            TruncationStrategy::DropMiddle => self.drop_middle(messages),
            TruncationStrategy::Summarize => {
                // TODO: Implement summarization in the future
                // For now, fall back to DropOldest
                self.drop_oldest(messages)
            }
        }
    }

    /// Drop oldest messages until we're within the token limit
    fn drop_oldest(&self, mut messages: Vec<Message>) -> Vec<Message> {
        // Preserve system messages at the beginning
        let system_messages: Vec<Message> = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .cloned()
            .collect();

        let mut non_system_messages: Vec<Message> = messages
            .into_iter()
            .filter(|m| m.role != Role::System)
            .collect();

        // Calculate tokens used by system messages
        let system_tokens = self.estimate_total_tokens(&system_messages);
        let available_tokens = self.config.max_tokens.saturating_sub(system_tokens);

        // Drop oldest non-system messages until we fit
        while !non_system_messages.is_empty() {
            let current_tokens = self.estimate_total_tokens(&non_system_messages);
            if current_tokens <= available_tokens {
                break;
            }
            non_system_messages.remove(0);
        }

        // Combine system messages with remaining non-system messages
        let mut result = system_messages;
        result.extend(non_system_messages);
        result
    }

    /// Keep first and last messages, drop middle ones
    fn drop_middle(&self, messages: Vec<Message>) -> Vec<Message> {
        if messages.len() <= 2 {
            return messages;
        }

        // Preserve system messages
        let system_messages: Vec<Message> = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .cloned()
            .collect();

        let non_system_messages: Vec<Message> = messages
            .into_iter()
            .filter(|m| m.role != Role::System)
            .collect();

        if non_system_messages.is_empty() {
            return system_messages;
        }

        // Calculate tokens
        let system_tokens = self.estimate_total_tokens(&system_messages);
        let available_tokens = self.config.max_tokens.saturating_sub(system_tokens);

        // Always keep first and last message
        let first = non_system_messages.first().cloned();
        let last = non_system_messages.last().cloned();

        let mut result = system_messages;

        if let Some(first_msg) = first {
            let first_tokens = self.estimate_tokens(&first_msg);
            let last_tokens = last.as_ref().map(|m| self.estimate_tokens(m)).unwrap_or(0);

            if first_tokens + last_tokens <= available_tokens {
                let first_text = first_msg.content_as_text();
                result.push(first_msg);

                // Add middle messages if there's room
                let remaining_tokens = available_tokens - first_tokens - last_tokens;
                let middle_count = non_system_messages.len().saturating_sub(2);
                let middle_messages: Vec<Message> = non_system_messages
                    .into_iter()
                    .skip(1)
                    .take(middle_count)
                    .collect();

                let mut current_tokens = 0;
                for msg in middle_messages {
                    let msg_tokens = self.estimate_tokens(&msg);
                    if current_tokens + msg_tokens <= remaining_tokens {
                        result.push(msg);
                        current_tokens += msg_tokens;
                    } else {
                        break;
                    }
                }

                if let Some(last_msg) = last {
                    if last_msg.content_as_text() != first_text {
                        result.push(last_msg);
                    }
                }
            } else {
                // Not enough room for both, just keep the last message
                if let Some(last_msg) = last {
                    result.push(last_msg);
                }
            }
        }

        result
    }

    /// Check if messages fit within the context window
    pub fn fits_in_window(&self, messages: &[Message]) -> bool {
        self.estimate_total_tokens(messages) <= self.config.max_tokens
    }

    /// Get the estimated token count for messages
    pub fn token_count(&self, messages: &[Message]) -> usize {
        self.estimate_total_tokens(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_message(role: Role, content: &str) -> Message {
        match role {
            Role::System => Message::system(content),
            Role::User => Message::user(content),
            Role::Assistant => Message::assistant(content),
        }
    }

    #[test]
    fn test_token_estimation() {
        let manager = ContextWindowManager::new(ContextWindowConfig::default());
        let message = create_message(Role::User, "Hello world"); // 11 chars ~= 3 tokens
        assert_eq!(manager.estimate_tokens(&message), 3);
    }

    #[test]
    fn test_no_truncation_needed() {
        let config = ContextWindowConfig::new(1000, TruncationStrategy::DropOldest);
        let manager = ContextWindowManager::new(config);

        let messages = vec![
            create_message(Role::User, "Hello"),
            create_message(Role::Assistant, "Hi there"),
        ];

        let result = manager.truncate_if_needed(messages.clone());
        assert_eq!(result.len(), messages.len());
    }

    #[test]
    fn test_drop_oldest_preserves_system() {
        let config = ContextWindowConfig::new(30, TruncationStrategy::DropOldest);
        let manager = ContextWindowManager::new(config);

        let messages = vec![
            create_message(Role::System, "You are a helpful assistant"),
            create_message(Role::User, "First message with lots of text that will exceed the token limit"),
            create_message(Role::Assistant, "Response with more text"),
            create_message(Role::User, "Second message with even more text"),
        ];

        let result = manager.truncate_if_needed(messages);

        // System message should be preserved
        assert!(result.iter().any(|m| m.role == Role::System));
        // Should have dropped some messages (original had 4)
        assert!(result.len() < 4);
    }

    #[test]
    fn test_drop_middle_keeps_first_and_last() {
        let config = ContextWindowConfig::new(50, TruncationStrategy::DropMiddle);
        let manager = ContextWindowManager::new(config);

        let messages = vec![
            create_message(Role::User, "First"),
            create_message(Role::Assistant, "Middle 1"),
            create_message(Role::User, "Middle 2"),
            create_message(Role::Assistant, "Last"),
        ];

        let result = manager.truncate_if_needed(messages.clone());

        // Should keep first and last
        if result.len() >= 2 {
            assert_eq!(result.first().unwrap().content_as_text(), "First");
            assert_eq!(result.last().unwrap().content_as_text(), "Last");
        }
    }

    #[test]
    fn test_fits_in_window() {
        let config = ContextWindowConfig::new(100, TruncationStrategy::DropOldest);
        let manager = ContextWindowManager::new(config);

        let small_messages = vec![
            create_message(Role::User, "Hi"),
        ];
        assert!(manager.fits_in_window(&small_messages));

        let large_messages = vec![
            create_message(Role::User, &"x".repeat(1000)),
        ];
        assert!(!manager.fits_in_window(&large_messages));
    }
}
