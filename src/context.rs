use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Context manager for agent state and shared data
#[derive(Clone)]
pub struct ContextManager {
    inner: Arc<RwLock<ContextInner>>,
}

struct ContextInner {
    variables: HashMap<String, Value>,
    metadata: HashMap<String, String>,
    todos: Vec<Todo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: Priority,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ContextInner {
                variables: HashMap::new(),
                metadata: HashMap::new(),
                todos: Vec::new(),
            })),
        }
    }

    /// Set a context variable
    pub async fn set(&self, key: impl Into<String>, value: Value) {
        let mut inner = self.inner.write().await;
        inner.variables.insert(key.into(), value);
    }

    /// Get a context variable
    pub async fn get(&self, key: &str) -> Option<Value> {
        let inner = self.inner.read().await;
        inner.variables.get(key).cloned()
    }

    /// Remove a context variable
    pub async fn remove(&self, key: &str) -> Option<Value> {
        let mut inner = self.inner.write().await;
        inner.variables.remove(key)
    }

    /// Set metadata
    pub async fn set_metadata(&self, key: impl Into<String>, value: impl Into<String>) {
        let mut inner = self.inner.write().await;
        inner.metadata.insert(key.into(), value.into());
    }

    /// Get metadata
    pub async fn get_metadata(&self, key: &str) -> Option<String> {
        let inner = self.inner.read().await;
        inner.metadata.get(key).cloned()
    }

    /// Add a todo item
    pub async fn add_todo(&self, content: impl Into<String>, priority: Priority) -> String {
        let mut inner = self.inner.write().await;
        let id = format!("todo_{}", inner.todos.len() + 1);
        let todo = Todo {
            id: id.clone(),
            content: content.into(),
            status: TodoStatus::Pending,
            priority,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            completed_at: None,
        };
        inner.todos.push(todo);
        id
    }

    /// Update todo status
    pub async fn update_todo_status(&self, id: &str, status: TodoStatus) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(todo) = inner.todos.iter_mut().find(|t| t.id == id) {
            todo.status = status;
            if status == TodoStatus::Completed {
                todo.completed_at = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                );
            }
            true
        } else {
            false
        }
    }

    /// Get all todos
    pub async fn todos(&self) -> Vec<Todo> {
        let inner = self.inner.read().await;
        inner.todos.clone()
    }

    /// Get pending todos
    pub async fn pending_todos(&self) -> Vec<Todo> {
        let inner = self.inner.read().await;
        inner
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::Pending || t.status == TodoStatus::InProgress)
            .cloned()
            .collect()
    }

    /// Export context as JSON
    pub async fn export(&self) -> Value {
        let inner = self.inner.read().await;
        serde_json::json!({
            "variables": inner.variables,
            "metadata": inner.metadata,
            "todos": inner.todos,
        })
    }

    /// Clear all context
    pub async fn clear(&self) {
        let mut inner = self.inner.write().await;
        inner.variables.clear();
        inner.metadata.clear();
        inner.todos.clear();
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}
