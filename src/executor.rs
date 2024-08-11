use crate::error::{NightError, NightResult};
use crate::scheduler::Scheduler;
use crate::task::{Runnable, Task, TaskId, TaskStatus};
use crate::task_manager::TaskStatusManager;
use std::sync::Arc;
use tokio::task;
use tokio::time::Duration;

pub struct Executor<T: TaskStatusManager + 'static> {
    scheduler: Arc<Scheduler>,
    task_manager: Arc<T>,
    concurrency: usize,
}

impl<T: TaskStatusManager + 'static> Executor<T> {
    pub fn new(scheduler: Arc<Scheduler>, task_manager: Arc<T>, concurrency: usize) -> Self {
        Executor {
            scheduler,
            task_manager,
            concurrency,
        }
    }

    pub async fn run(&self) -> NightResult<()> {
        let mut handles = vec![];

        for _ in 0..self.concurrency {
            let scheduler = Arc::clone(&self.scheduler);
            let task_manager = Arc::clone(&self.task_manager);
            let handle = task::spawn(async move {
                loop {
                    match scheduler.next_task().await {
                        Ok(Some(task)) => {
                            if let Err(e) = Self::execute_task(
                                Arc::clone(&task_manager),
                                Arc::clone(&scheduler),
                                task,
                            )
                            .await
                            {
                                eprintln!("Task execution error: {}", e);
                            }
                        }
                        Ok(None) => {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        Err(e) => {
                            eprintln!("Error getting next task: {}", e);
                        }
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle
                .await
                .map_err(|e| NightError::TaskExecutionError(e.to_string()))?;
        }

        Ok(())
    }

    async fn execute_task(
        task_manager: Arc<T>,
        scheduler: Arc<Scheduler>,
        task: Arc<Task>,
    ) -> NightResult<()> {
        let task_id = task.id();

        if task.has_timed_out() {
            task_manager
                .update_task_status(task_id, TaskStatus::Failed)
                .await?;
            return Err(NightError::TaskExecutionError(format!(
                "Task {} timed out",
                task_id
            )));
        }

        task_manager
            .update_task_status(task_id, TaskStatus::Running)
            .await?;

        let result = if let Some(timeout_duration) = task.timeout() {
            tokio::time::timeout(timeout_duration, Self::run_task(task.clone())).await
        } else {
            Ok(Self::run_task(task.clone()).await)
        };

        match result {
            Ok(Ok(_)) => {
                task_manager
                    .update_task_status(task_id, TaskStatus::Completed)
                    .await?;
                scheduler.complete_task(task_id).await?;
                Ok(())
            }
            Ok(Err(e)) => {
                Self::handle_task_failure(task_manager, scheduler, task, task_id, e).await
            }
            Err(_) => Self::handle_task_timeout(task_manager, scheduler, task, task_id).await,
        }
    }

    async fn handle_task_failure(
        task_manager: Arc<T>,
        scheduler: Arc<Scheduler>,
        task: Arc<Task>,
        task_id: TaskId,
        error: NightError,
    ) -> NightResult<()> {
        if task.can_retry() {
            let mut task = (*task).clone();
            task.increment_retries();
            task_manager
                .update_task_status(task_id, TaskStatus::Queued)
                .await?;
            scheduler.add_task(task).await?;
            Ok(())
        } else {
            task_manager
                .update_task_status(task_id, TaskStatus::Failed)
                .await?;
            Err(NightError::TaskExecutionError(format!(
                "Task {} failed: {}",
                task_id, error
            )))
        }
    }

    async fn handle_task_timeout(
        task_manager: Arc<T>,
        scheduler: Arc<Scheduler>,
        task: Arc<Task>,
        task_id: TaskId,
    ) -> NightResult<()> {
        if task.can_retry() {
            let mut task = (*task).clone();
            task.increment_retries();
            task_manager
                .update_task_status(task_id, TaskStatus::Queued)
                .await?;
            scheduler.add_task(task).await?;
            Ok(())
        } else {
            task_manager
                .update_task_status(task_id, TaskStatus::Failed)
                .await?;
            Err(NightError::TaskExecutionError(format!(
                "Task {} timed out and exceeded retry limit",
                task_id
            )))
        }
    }

    async fn run_task(task: Arc<Task>) -> NightResult<()> {
        let runnable = task
            .data()
            .downcast_ref::<Box<dyn Runnable>>()
            .ok_or_else(|| {
                NightError::TaskExecutionError("Failed to downcast task data".to_string())
            })?;
        runnable
            .run()
            .map_err(|e| NightError::TaskExecutionError(e.to_string()))
    }
}
