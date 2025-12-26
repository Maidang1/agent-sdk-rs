use crate::Message;

pub struct Memory {
    messages: Vec<Message>,
    max_messages: Option<usize>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_messages: None,
        }
    }

    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = Some(max);
        self
    }

    pub fn add(&mut self, message: Message) {
        self.messages.push(message);
        self.trim();
    }

    pub fn add_many(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.messages.extend(messages);
        self.trim();
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    fn trim(&mut self) {
        if let Some(max) = self.max_messages {
            if self.messages.len() > max {
                // Keep system message if present
                let has_system = self
                    .messages
                    .first()
                    .map(|m| matches!(m.role, crate::MessageRole::System))
                    .unwrap_or(false);

                if has_system && self.messages.len() > 1 {
                    let system = self.messages.remove(0);
                    let keep = max.saturating_sub(1);
                    let drain_count = self.messages.len().saturating_sub(keep);
                    self.messages.drain(0..drain_count);
                    self.messages.insert(0, system);
                } else {
                    let drain_count = self.messages.len().saturating_sub(max);
                    self.messages.drain(0..drain_count);
                }
            }
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
