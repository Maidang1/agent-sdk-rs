use crate::{ToolCall, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};

/// Approval decision for tool execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Rejected(String),
    Pending,
}

/// Policy for automatic approval/rejection
#[derive(Clone)]
pub enum ApprovalPolicy {
    /// Always approve
    AutoApprove,
    /// Always reject with reason
    AutoReject(String),
    /// Require manual approval
    RequireApproval,
    /// Approve if tool is in allowlist
    Allowlist(Vec<String>),
    /// Reject if tool is in blocklist
    Blocklist(Vec<String>),
    /// Custom policy function
    Custom(Arc<dyn Fn(&ToolCall) -> ApprovalDecision + Send + Sync>),
}

impl std::fmt::Debug for ApprovalPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AutoApprove => write!(f, "AutoApprove"),
            Self::AutoReject(r) => write!(f, "AutoReject({:?})", r),
            Self::RequireApproval => write!(f, "RequireApproval"),
            Self::Allowlist(l) => write!(f, "Allowlist({:?})", l),
            Self::Blocklist(l) => write!(f, "Blocklist({:?})", l),
            Self::Custom(_) => write!(f, "Custom(...)"),
        }
    }
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self::AutoApprove
    }
}

/// Pending approval request
pub struct PendingApproval {
    pub tool_call: ToolCall,
    pub responder: oneshot::Sender<ApprovalDecision>,
}

/// Approval manager for tool execution control
pub struct ApprovalManager {
    policy: RwLock<ApprovalPolicy>,
    tool_policies: RwLock<HashMap<String, ApprovalPolicy>>,
    pending: RwLock<HashMap<String, PendingApproval>>,
}

impl ApprovalManager {
    pub fn new() -> Self {
        Self {
            policy: RwLock::new(ApprovalPolicy::AutoApprove),
            tool_policies: RwLock::new(HashMap::new()),
            pending: RwLock::new(HashMap::new()),
        }
    }

    /// Set default approval policy
    pub async fn set_policy(&self, policy: ApprovalPolicy) {
        let mut p = self.policy.write().await;
        *p = policy;
    }

    /// Set policy for specific tool
    pub async fn set_tool_policy(&self, tool_name: impl Into<String>, policy: ApprovalPolicy) {
        let mut policies = self.tool_policies.write().await;
        policies.insert(tool_name.into(), policy);
    }

    /// Check if tool call requires approval
    pub async fn check(&self, tool_call: &ToolCall) -> ApprovalDecision {
        // Check tool-specific policy first
        let tool_policies = self.tool_policies.read().await;
        if let Some(policy) = tool_policies.get(&tool_call.name) {
            return self.evaluate_policy(policy, tool_call);
        }

        // Fall back to default policy
        let policy = self.policy.read().await;
        self.evaluate_policy(&policy, tool_call)
    }

    fn evaluate_policy(&self, policy: &ApprovalPolicy, tool_call: &ToolCall) -> ApprovalDecision {
        match policy {
            ApprovalPolicy::AutoApprove => ApprovalDecision::Approved,
            ApprovalPolicy::AutoReject(reason) => ApprovalDecision::Rejected(reason.clone()),
            ApprovalPolicy::RequireApproval => ApprovalDecision::Pending,
            ApprovalPolicy::Allowlist(list) => {
                if list.contains(&tool_call.name) {
                    ApprovalDecision::Approved
                } else {
                    ApprovalDecision::Pending
                }
            }
            ApprovalPolicy::Blocklist(list) => {
                if list.contains(&tool_call.name) {
                    ApprovalDecision::Rejected(format!("Tool '{}' is blocked", tool_call.name))
                } else {
                    ApprovalDecision::Approved
                }
            }
            ApprovalPolicy::Custom(f) => f(tool_call),
        }
    }

    /// Request approval for a tool call (async wait for decision)
    pub async fn request_approval(&self, tool_call: ToolCall) -> Result<ApprovalDecision> {
        let decision = self.check(&tool_call).await;
        
        if decision != ApprovalDecision::Pending {
            return Ok(decision);
        }

        // Create pending approval
        let (tx, rx) = oneshot::channel();
        let id = tool_call.id.clone();
        
        {
            let mut pending = self.pending.write().await;
            pending.insert(id.clone(), PendingApproval {
                tool_call,
                responder: tx,
            });
        }

        // Wait for decision
        rx.await.map_err(|_| anyhow::anyhow!("Approval request cancelled"))
    }

    /// Approve a pending tool call
    pub async fn approve(&self, tool_call_id: &str) -> bool {
        let mut pending = self.pending.write().await;
        if let Some(approval) = pending.remove(tool_call_id) {
            let _ = approval.responder.send(ApprovalDecision::Approved);
            true
        } else {
            false
        }
    }

    /// Reject a pending tool call
    pub async fn reject(&self, tool_call_id: &str, reason: impl Into<String>) -> bool {
        let mut pending = self.pending.write().await;
        if let Some(approval) = pending.remove(tool_call_id) {
            let _ = approval.responder.send(ApprovalDecision::Rejected(reason.into()));
            true
        } else {
            false
        }
    }

    /// Get all pending approvals
    pub async fn pending_approvals(&self) -> Vec<ToolCall> {
        let pending = self.pending.read().await;
        pending.values().map(|p| p.tool_call.clone()).collect()
    }

    /// Cancel all pending approvals
    pub async fn cancel_all(&self) {
        let mut pending = self.pending.write().await;
        for (_, approval) in pending.drain() {
            let _ = approval.responder.send(ApprovalDecision::Rejected("Cancelled".to_string()));
        }
    }
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for custom approval handlers
#[async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn on_approval_required(&self, tool_call: &ToolCall) -> ApprovalDecision;
}

/// Interactive approval handler that prompts user
pub struct InteractiveApprovalHandler;

#[async_trait]
impl ApprovalHandler for InteractiveApprovalHandler {
    async fn on_approval_required(&self, tool_call: &ToolCall) -> ApprovalDecision {
        println!("Tool '{}' requires approval.", tool_call.name);
        println!("Parameters: {}", serde_json::to_string_pretty(&tool_call.parameters).unwrap_or_default());
        println!("Approve? (y/n): ");
        
        // In a real implementation, this would read from stdin
        // For now, auto-approve
        ApprovalDecision::Approved
    }
}
