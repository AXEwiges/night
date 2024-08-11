use crate::error::NightResult;
use crate::task::{TaskId, TaskStatus};
use async_trait::async_trait;

#[async_trait]
pub trait TaskStatusManager: Send + Sync {
    async fn update_task_status(&self, task_id: TaskId, status: TaskStatus) -> NightResult<()>;
}
