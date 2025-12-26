use crate::event::{AgentEvent, ControlEvent, EventBus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

/// Trigger condition for scheduled tasks
#[derive(Clone)]
pub enum Trigger {
    /// Trigger after N iterations
    AfterIterations(usize),
    /// Trigger after duration
    AfterDuration(Duration),
    /// Trigger at specific interval
    Interval(Duration),
    /// Trigger on specific event pattern
    OnEvent(String),
    /// Custom condition
    Custom(Arc<dyn Fn(&SchedulerContext) -> bool + Send + Sync>),
}

impl std::fmt::Debug for Trigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AfterIterations(n) => write!(f, "AfterIterations({})", n),
            Self::AfterDuration(d) => write!(f, "AfterDuration({:?})", d),
            Self::Interval(d) => write!(f, "Interval({:?})", d),
            Self::OnEvent(s) => write!(f, "OnEvent({:?})", s),
            Self::Custom(_) => write!(f, "Custom(...)"),
        }
    }
}

/// Action to perform when triggered
#[derive(Clone)]
pub enum ScheduledAction {
    /// Send a reminder message
    Reminder(String),
    /// Pause the agent
    Pause,
    /// Emit a custom event
    EmitEvent(AgentEvent),
    /// Execute a callback
    Callback(Arc<dyn Fn() + Send + Sync>),
}

/// Context for scheduler decisions
#[derive(Debug, Clone)]
pub struct SchedulerContext {
    pub iteration_count: usize,
    pub elapsed: Duration,
    pub last_event: Option<String>,
}

/// Scheduled task
pub struct ScheduledTask {
    pub id: String,
    pub trigger: Trigger,
    pub action: ScheduledAction,
    pub repeat: bool,
    pub last_triggered: Option<Instant>,
}

/// Scheduler for managing timed and conditional tasks
pub struct Scheduler {
    tasks: Arc<RwLock<HashMap<String, ScheduledTask>>>,
    event_bus: Arc<EventBus>,
    context: Arc<RwLock<SchedulerContext>>,
}

impl Scheduler {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            context: Arc::new(RwLock::new(SchedulerContext {
                iteration_count: 0,
                elapsed: Duration::ZERO,
                last_event: None,
            })),
        }
    }

    /// Add a scheduled task
    pub async fn add_task(&self, task: ScheduledTask) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
    }

    /// Remove a scheduled task
    pub async fn remove_task(&self, id: &str) -> Option<ScheduledTask> {
        let mut tasks = self.tasks.write().await;
        tasks.remove(id)
    }

    /// Schedule a reminder after N iterations
    pub async fn remind_after_iterations(&self, id: impl Into<String>, iterations: usize, message: impl Into<String>) {
        let task = ScheduledTask {
            id: id.into(),
            trigger: Trigger::AfterIterations(iterations),
            action: ScheduledAction::Reminder(message.into()),
            repeat: false,
            last_triggered: None,
        };
        self.add_task(task).await;
    }

    /// Schedule a reminder at interval
    pub async fn remind_at_interval(&self, id: impl Into<String>, interval: Duration, message: impl Into<String>) {
        let task = ScheduledTask {
            id: id.into(),
            trigger: Trigger::Interval(interval),
            action: ScheduledAction::Reminder(message.into()),
            repeat: true,
            last_triggered: None,
        };
        self.add_task(task).await;
    }

    /// Update iteration count
    pub async fn tick_iteration(&self) {
        let mut ctx = self.context.write().await;
        ctx.iteration_count += 1;
    }

    /// Update elapsed time
    pub async fn update_elapsed(&self, elapsed: Duration) {
        let mut ctx = self.context.write().await;
        ctx.elapsed = elapsed;
    }

    /// Record last event
    pub async fn record_event(&self, event_type: impl Into<String>) {
        let mut ctx = self.context.write().await;
        ctx.last_event = Some(event_type.into());
    }

    /// Check and execute triggered tasks
    pub async fn check_triggers(&self, agent_id: &str) -> Vec<ScheduledAction> {
        let ctx = self.context.read().await.clone();
        let mut tasks = self.tasks.write().await;
        let mut triggered_actions = Vec::new();
        let mut to_remove = Vec::new();

        for (id, task) in tasks.iter_mut() {
            let should_trigger = match &task.trigger {
                Trigger::AfterIterations(n) => ctx.iteration_count >= *n && task.last_triggered.is_none(),
                Trigger::AfterDuration(d) => ctx.elapsed >= *d && task.last_triggered.is_none(),
                Trigger::Interval(d) => {
                    task.last_triggered
                        .map(|t| t.elapsed() >= *d)
                        .unwrap_or(true)
                }
                Trigger::OnEvent(pattern) => {
                    ctx.last_event
                        .as_ref()
                        .map(|e| e.contains(pattern))
                        .unwrap_or(false)
                }
                Trigger::Custom(f) => f(&ctx),
            };

            if should_trigger {
                task.last_triggered = Some(Instant::now());
                triggered_actions.push(task.action.clone());

                // Execute action
                match &task.action {
                    ScheduledAction::Reminder(msg) => {
                        self.event_bus.publish(AgentEvent::Control(ControlEvent::Interrupt {
                            agent_id: agent_id.to_string(),
                            message: msg.clone(),
                        }));
                    }
                    ScheduledAction::Pause => {
                        self.event_bus.publish(AgentEvent::Control(ControlEvent::Pause {
                            agent_id: agent_id.to_string(),
                        }));
                    }
                    ScheduledAction::EmitEvent(event) => {
                        self.event_bus.publish(event.clone());
                    }
                    ScheduledAction::Callback(f) => {
                        f();
                    }
                }

                if !task.repeat {
                    to_remove.push(id.clone());
                }
            }
        }

        for id in to_remove {
            tasks.remove(&id);
        }

        triggered_actions
    }

    /// Get current context
    pub async fn context(&self) -> SchedulerContext {
        self.context.read().await.clone()
    }

    /// Reset scheduler state
    pub async fn reset(&self) {
        let mut ctx = self.context.write().await;
        ctx.iteration_count = 0;
        ctx.elapsed = Duration::ZERO;
        ctx.last_event = None;
    }
}

impl std::fmt::Debug for ScheduledAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reminder(msg) => write!(f, "Reminder({:?})", msg),
            Self::Pause => write!(f, "Pause"),
            Self::EmitEvent(e) => write!(f, "EmitEvent({:?})", e),
            Self::Callback(_) => write!(f, "Callback(...)"),
        }
    }
}

impl std::fmt::Debug for ScheduledTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScheduledTask")
            .field("id", &self.id)
            .field("trigger", &self.trigger)
            .field("action", &self.action)
            .field("repeat", &self.repeat)
            .finish()
    }
}
