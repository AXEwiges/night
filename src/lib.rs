pub mod distributed;
pub mod error;
pub mod executor;
pub mod persistence;
pub mod queue;
pub mod scheduler;
pub mod task;
pub mod task_manager;
pub mod utils;

use crate::distributed::{node::Node, DistributedSystem};
use crate::error::{NightError, NightResult};
use crate::executor::Executor;
use crate::persistence::Persistence;
use crate::queue::TaskQueue;
use crate::scheduler::Scheduler;
use crate::task::{Task, TaskId, TaskStatus};
use crate::task_manager::TaskStatusManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

#[allow(dead_code)]
pub struct Night {
    queue: Arc<TaskQueue>,
    scheduler: Arc<Scheduler>,
    executor: RwLock<Option<Arc<Executor<Self>>>>,
    persistence: Arc<Persistence>,
    distributed_system: Arc<Mutex<Option<DistributedSystem>>>,
    task_statuses: RwLock<HashMap<TaskId, TaskStatus>>,
}

#[async_trait::async_trait]
impl TaskStatusManager for Night {
    async fn update_task_status(&self, task_id: TaskId, status: TaskStatus) -> NightResult<()> {
        self.task_statuses.write().await.insert(task_id, status);
        Ok(())
    }
}

impl Night {
    pub fn new(concurrency: usize, persistence_path: &str) -> Arc<Self> {
        let queue = Arc::new(TaskQueue::new());
        let scheduler = Arc::new(Scheduler::new(Arc::clone(&queue)));
        let persistence = Arc::new(Persistence::new(persistence_path.to_string()));

        let night = Arc::new(Self {
            queue,
            scheduler: Arc::clone(&scheduler),
            executor: RwLock::new(None),
            persistence,
            distributed_system: Arc::new(Mutex::new(None)),
            task_statuses: RwLock::new(HashMap::new()),
        });

        let executor = Arc::new(Executor::new(
            Arc::clone(&scheduler),
            Arc::clone(&night),
            concurrency,
        ));

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                *night.executor.write().await = Some(executor);
            });
        });

        night
    }

    pub async fn start(self: &Arc<Self>) -> NightResult<()> {
        // Load persisted state
        let tasks = self.persistence.load_state().await?;
        for (_, task) in tasks {
            self.scheduler.add_task(task).await?;
        }

        // Start the executor
        let executor = self
            .executor
            .read()
            .await
            .clone()
            .expect("Executor not initialized");
        // let night = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(e) = executor.run().await {
                eprintln!("Executor error: {}", e);
            }
        });

        Ok(())
    }

    pub async fn submit_task(&self, task: Task) -> NightResult<()> {
        let task_id = task.id();
        // self.task_statuses.write().await.insert(task_id, TaskStatus::Queued);
        self.update_task_status(task_id, TaskStatus::Queued).await?;
        self.scheduler.add_task(task).await?;
        Ok(())
    }

    pub async fn get_task_status(&self, task_id: TaskId) -> NightResult<Option<String>> {
        let statuses = self.task_statuses.read().await;
        Ok(statuses.get(&task_id).map(|status| status.to_string()))
    }

    pub(crate) async fn update_task_status(
        &self,
        task_id: TaskId,
        status: TaskStatus,
    ) -> NightResult<()> {
        self.task_statuses.write().await.insert(task_id, status);
        Ok(())
    }

    pub async fn enable_distributed_mode(&self, node: Arc<Node>) -> NightResult<()> {
        let mut distributed_system = self.distributed_system.lock().await;
        if distributed_system.is_some() {
            return Err(NightError::DistributedError(
                "Distributed mode already enabled".to_string(),
            ));
        }

        let new_system = DistributedSystem::new(node);
        *distributed_system = Some(new_system);

        let ds_clone = Arc::clone(&self.distributed_system);

        tokio::spawn(async move {
            loop {
                let system = ds_clone.lock().await;
                if let Some(ref ds) = *system {
                    if let Err(e) = ds.start_server().await {
                        eprintln!("Error starting distributed server: {}", e);
                        break;
                    }
                } else {
                    break;
                }
                drop(system);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });

        Ok(())
    }

    pub async fn add_node(&self, node: Arc<Node>) -> NightResult<()> {
        let distributed_system = self.distributed_system.lock().await;
        if let Some(ds) = distributed_system.as_ref() {
            ds.add_node(node).await?;
            Ok(())
        } else {
            Err(NightError::DistributedError(
                "Distributed mode not enabled".to_string(),
            ))
        }
    }

    pub async fn remove_node(&self, node_id: &str) -> NightResult<()> {
        let distributed_system = self.distributed_system.lock().await;
        if let Some(ds) = distributed_system.as_ref() {
            ds.remove_node(&node_id.to_string()).await?;
            Ok(())
        } else {
            Err(NightError::DistributedError(
                "Distributed mode not enabled".to_string(),
            ))
        }
    }
}
