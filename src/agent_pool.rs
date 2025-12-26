use crate::event::{AgentEvent, EventBus, ProgressEvent};
use crate::llm::LLMClient;
use crate::runtime::{Runtime, RuntimeOptions};
use crate::{Message, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent state in the pool
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    Idle,
    Running,
    Paused,
    Completed,
    Error,
}

/// Agent metadata
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub state: AgentState,
    pub parent_id: Option<String>,
    pub children_ids: Vec<String>,
    pub created_at: u64,
}

/// Agent pool for managing multiple agents
pub struct AgentPool<L: LLMClient + Clone + 'static> {
    agents: Arc<RwLock<HashMap<String, AgentEntry<L>>>>,
    event_bus: Arc<EventBus>,
    default_options: RuntimeOptions,
}

struct AgentEntry<L: LLMClient> {
    runtime: Runtime<L>,
    info: AgentInfo,
}

impl<L: LLMClient + Clone + 'static> AgentPool<L> {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            default_options: RuntimeOptions::default(),
        }
    }

    pub fn with_default_options(mut self, options: RuntimeOptions) -> Self {
        self.default_options = options;
        self
    }

    /// Create a new agent in the pool
    pub async fn create_agent(
        &self,
        id: impl Into<String>,
        name: impl Into<String>,
        llm: L,
        options: Option<RuntimeOptions>,
    ) -> Result<String> {
        let id = id.into();
        let name = name.into();
        let options = options.unwrap_or_else(|| self.default_options.clone());

        let runtime = Runtime::new(llm).with_options(options);
        let info = AgentInfo {
            id: id.clone(),
            name,
            state: AgentState::Idle,
            parent_id: None,
            children_ids: Vec::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let mut agents = self.agents.write().await;
        agents.insert(id.clone(), AgentEntry { runtime, info });

        Ok(id)
    }

    /// Fork an agent (create child with same context)
    pub async fn fork_agent(&self, parent_id: &str, new_id: impl Into<String>, llm: L) -> Result<String> {
        let new_id = new_id.into();
        let agents = self.agents.read().await;
        
        let parent = agents
            .get(parent_id)
            .ok_or_else(|| anyhow::anyhow!("Parent agent not found"))?;

        // Clone parent's messages
        let messages: Vec<Message> = parent.runtime.memory().messages().to_vec();
        drop(agents);

        // Create new agent with parent's context
        let mut runtime = Runtime::new(llm).with_options(self.default_options.clone());
        runtime.memory_mut().add_many(messages);

        let info = AgentInfo {
            id: new_id.clone(),
            name: format!("fork_of_{}", parent_id),
            state: AgentState::Idle,
            parent_id: Some(parent_id.to_string()),
            children_ids: Vec::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let mut agents = self.agents.write().await;
        
        // Update parent's children list
        if let Some(parent) = agents.get_mut(parent_id) {
            parent.info.children_ids.push(new_id.clone());
        }

        agents.insert(new_id.clone(), AgentEntry { runtime, info });

        Ok(new_id)
    }

    /// Run an agent with input
    pub async fn run_agent(&self, id: &str, input: impl Into<String>) -> Result<String> {
        let input = input.into();
        
        // Update state to running
        {
            let mut agents = self.agents.write().await;
            if let Some(entry) = agents.get_mut(id) {
                entry.info.state = AgentState::Running;
            }
        }

        self.event_bus.publish(AgentEvent::Progress(ProgressEvent::Started {
            agent_id: id.to_string(),
            session_id: format!("session_{}", id),
        }));

        // Run the agent
        let result = {
            let mut agents = self.agents.write().await;
            let entry = agents
                .get_mut(id)
                .ok_or_else(|| anyhow::anyhow!("Agent not found"))?;
            entry.runtime.run(&input).await
        };

        // Update state based on result
        {
            let mut agents = self.agents.write().await;
            if let Some(entry) = agents.get_mut(id) {
                entry.info.state = if result.is_ok() {
                    AgentState::Completed
                } else {
                    AgentState::Error
                };
            }
        }

        match &result {
            Ok(response) => {
                self.event_bus.publish(AgentEvent::Progress(ProgressEvent::Completed {
                    agent_id: id.to_string(),
                    result: response.clone(),
                }));
            }
            Err(e) => {
                self.event_bus.publish(AgentEvent::Progress(ProgressEvent::Error {
                    agent_id: id.to_string(),
                    error: e.to_string(),
                }));
            }
        }

        result
    }

    /// Get agent info
    pub async fn get_agent_info(&self, id: &str) -> Option<AgentInfo> {
        let agents = self.agents.read().await;
        agents.get(id).map(|e| e.info.clone())
    }

    /// List all agents
    pub async fn list_agents(&self) -> Vec<AgentInfo> {
        let agents = self.agents.read().await;
        agents.values().map(|e| e.info.clone()).collect()
    }

    /// Remove an agent
    pub async fn remove_agent(&self, id: &str) -> bool {
        let mut agents = self.agents.write().await;
        agents.remove(id).is_some()
    }

    /// Get agent lineage (ancestors)
    pub async fn get_lineage(&self, id: &str) -> Vec<String> {
        let agents = self.agents.read().await;
        let mut lineage = Vec::new();
        let mut current_id = Some(id.to_string());

        while let Some(cid) = current_id {
            if let Some(entry) = agents.get(&cid) {
                lineage.push(cid);
                current_id = entry.info.parent_id.clone();
            } else {
                break;
            }
        }

        lineage.reverse();
        lineage
    }

    /// Register a tool for an agent
    pub async fn register_tool(&self, agent_id: &str, tool: Box<dyn crate::Tool>) -> Result<()> {
        let mut agents = self.agents.write().await;
        let entry = agents
            .get_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found"))?;
        entry.runtime.register_tool(tool);
        Ok(())
    }
}
