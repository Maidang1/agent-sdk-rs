use crate::event::{AgentEvent, EventBus, ProgressEvent};
use crate::Message;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Room message for multi-agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMessage {
    pub id: String,
    pub from_agent: String,
    pub to_agent: Option<String>, // None = broadcast
    pub content: String,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

/// Room for multi-agent collaboration
pub struct Room {
    id: String,
    name: String,
    members: Arc<RwLock<Vec<String>>>,
    messages: Arc<RwLock<VecDeque<RoomMessage>>>,
    event_bus: Arc<EventBus>,
    max_messages: usize,
}

impl Room {
    pub fn new(id: impl Into<String>, name: impl Into<String>, event_bus: Arc<EventBus>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            members: Arc::new(RwLock::new(Vec::new())),
            messages: Arc::new(RwLock::new(VecDeque::new())),
            event_bus,
            max_messages: 1000,
        }
    }

    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = max;
        self
    }

    /// Add an agent to the room
    pub async fn join(&self, agent_id: impl Into<String>) {
        let agent_id = agent_id.into();
        let mut members = self.members.write().await;
        if !members.contains(&agent_id) {
            members.push(agent_id.clone());
            
            // Broadcast join event
            self.event_bus.publish(AgentEvent::Progress(ProgressEvent::Message {
                agent_id: agent_id.clone(),
                message: Message::system(format!("Agent {} joined room {}", agent_id, self.name)),
            }));
        }
    }

    /// Remove an agent from the room
    pub async fn leave(&self, agent_id: &str) {
        let mut members = self.members.write().await;
        members.retain(|id| id != agent_id);
        
        self.event_bus.publish(AgentEvent::Progress(ProgressEvent::Message {
            agent_id: agent_id.to_string(),
            message: Message::system(format!("Agent {} left room {}", agent_id, self.name)),
        }));
    }

    /// Send a message to the room
    pub async fn send(&self, from_agent: impl Into<String>, content: impl Into<String>) -> String {
        let msg_id = format!("msg_{}_{}", self.id, uuid_simple());
        let message = RoomMessage {
            id: msg_id.clone(),
            from_agent: from_agent.into(),
            to_agent: None,
            content: content.into(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        };

        let mut messages = self.messages.write().await;
        messages.push_back(message.clone());
        
        // Trim old messages
        while messages.len() > self.max_messages {
            messages.pop_front();
        }

        // Broadcast to event bus
        self.event_bus.publish(AgentEvent::Progress(ProgressEvent::Message {
            agent_id: message.from_agent.clone(),
            message: Message::assistant(&message.content),
        }));

        msg_id
    }

    /// Send a direct message to specific agent
    pub async fn send_to(
        &self,
        from_agent: impl Into<String>,
        to_agent: impl Into<String>,
        content: impl Into<String>,
    ) -> String {
        let msg_id = format!("msg_{}_{}", self.id, uuid_simple());
        let message = RoomMessage {
            id: msg_id.clone(),
            from_agent: from_agent.into(),
            to_agent: Some(to_agent.into()),
            content: content.into(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        };

        let mut messages = self.messages.write().await;
        messages.push_back(message);

        msg_id
    }

    /// Get messages for an agent (broadcasts + direct messages)
    pub async fn get_messages_for(&self, agent_id: &str, limit: Option<usize>) -> Vec<RoomMessage> {
        let messages = self.messages.read().await;
        let filtered: Vec<_> = messages
            .iter()
            .filter(|m| {
                m.to_agent.is_none() || m.to_agent.as_ref() == Some(&agent_id.to_string())
            })
            .cloned()
            .collect();

        match limit {
            Some(n) => filtered.into_iter().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect(),
            None => filtered,
        }
    }

    /// Get all messages
    pub async fn get_all_messages(&self, limit: Option<usize>) -> Vec<RoomMessage> {
        let messages = self.messages.read().await;
        match limit {
            Some(n) => messages.iter().rev().take(n).cloned().collect::<Vec<_>>().into_iter().rev().collect(),
            None => messages.iter().cloned().collect(),
        }
    }

    /// Get room members
    pub async fn members(&self) -> Vec<String> {
        self.members.read().await.clone()
    }

    /// Get room ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get room name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Clear all messages
    pub async fn clear(&self) {
        let mut messages = self.messages.write().await;
        messages.clear();
    }
}

/// Simple UUID generator (for demo purposes)
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{:x}{:x}", duration.as_secs(), duration.subsec_nanos())
}

/// Room manager for multiple rooms
pub struct RoomManager {
    rooms: Arc<RwLock<HashMap<String, Arc<Room>>>>,
    event_bus: Arc<EventBus>,
}

impl RoomManager {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
        }
    }

    /// Create a new room
    pub async fn create_room(&self, id: impl Into<String>, name: impl Into<String>) -> Arc<Room> {
        let id = id.into();
        let room = Arc::new(Room::new(id.clone(), name, self.event_bus.clone()));
        
        let mut rooms = self.rooms.write().await;
        rooms.insert(id, room.clone());
        
        room
    }

    /// Get a room by ID
    pub async fn get_room(&self, id: &str) -> Option<Arc<Room>> {
        let rooms = self.rooms.read().await;
        rooms.get(id).cloned()
    }

    /// Remove a room
    pub async fn remove_room(&self, id: &str) -> bool {
        let mut rooms = self.rooms.write().await;
        rooms.remove(id).is_some()
    }

    /// List all rooms
    pub async fn list_rooms(&self) -> Vec<String> {
        let rooms = self.rooms.read().await;
        rooms.keys().cloned().collect()
    }
}
