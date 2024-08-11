use crate::error::NightResult;
use crate::task::TaskId;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::RwLock;

pub type NodeId = String;

#[derive(Debug, Clone)]
pub enum NodeState {
    Active,
    Inactive,
    Failed,
}

pub struct Node {
    id: NodeId,
    addr: SocketAddr,
    state: RwLock<NodeState>,
    tasks: RwLock<HashMap<TaskId, TaskStatus>>,
}

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl Node {
    pub fn new(id: NodeId, addr: SocketAddr) -> Self {
        Node {
            id,
            addr,
            state: RwLock::new(NodeState::Active),
            tasks: RwLock::new(HashMap::new()),
        }
    }

    pub fn id(&self) -> &NodeId {
        &self.id
    }

    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub async fn get_state(&self) -> NodeState {
        self.state.read().await.clone()
    }

    pub async fn set_state(&self, new_state: NodeState) {
        *self.state.write().await = new_state;
    }

    pub async fn add_task(&self, task_id: TaskId) -> NightResult<()> {
        self.tasks.write().await.insert(task_id, TaskStatus::Queued);
        Ok(())
    }

    pub async fn update_task_status(&self, task_id: TaskId, status: TaskStatus) -> NightResult<()> {
        if let Some(task_status) = self.tasks.write().await.get_mut(&task_id) {
            *task_status = status;
            Ok(())
        } else {
            Err(crate::error::NightError::TaskNotFound(format!(
                "Task {} not found on node {}",
                task_id, self.id
            )))
        }
    }

    pub async fn get_task_status(&self, task_id: &TaskId) -> Option<TaskStatus> {
        self.tasks.read().await.get(task_id).cloned()
    }

    pub async fn get_all_tasks(&self) -> HashMap<TaskId, TaskStatus> {
        self.tasks.read().await.clone()
    }
}
