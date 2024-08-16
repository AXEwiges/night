use crate::common::types::{TaskAddressMap, TaskConfig, TaskDependencyMap, TaskInfo, TaskStatus};
use crate::utils::error::{NightError, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::time::{interval, Duration};
use uuid::Uuid;

use super::communication::TaskCommunicator;

static GLOBAL_EXECUTION_ORDER: AtomicUsize = AtomicUsize::new(0);
const MAX_RETRY_ATTEMPTS: u32 = 5;
const RETRY_DELAY: Duration = Duration::from_secs(1);
const SEND_DELAY: Duration = Duration::from_micros(200);

pub struct Task {
    pub config: TaskConfig,
    pub status: Arc<Mutex<TaskStatus>>,
    pub start_time: Arc<Mutex<Option<DateTime<Utc>>>>,
    pub end_time: Arc<Mutex<Option<DateTime<Utc>>>>,
    pub execution_lock: Arc<AtomicBool>,
    pub dependency_status: Arc<Mutex<TaskDependencyMap>>,
    pub address_map: Arc<TaskAddressMap>,
    pub execution_order: Arc<Mutex<Option<usize>>>,
    pub communicator: Arc<TaskCommunicator>,
    pub dependent_tasks: Arc<Vec<Uuid>>,
}

impl Task {
    pub fn new(
        config: TaskConfig,
        address_map: Arc<TaskAddressMap>,
        dependent_tasks: Vec<Uuid>,
    ) -> Self {
        let dependency_status: Arc<Mutex<HashMap<Uuid, bool>>> = Arc::new(Mutex::new(
            config.dependencies.iter().map(|&id| (id, false)).collect(),
        ));

        let task_id = config.id;
        let dependency_handler = {
            let dependency_status = Arc::clone(&dependency_status);
            move |completed_task_id| {
                let dependency_status = Arc::clone(&dependency_status);
                Box::pin(async move {
                    let mut dependencies = dependency_status.lock().unwrap();
                    if let Some(status) = dependencies.get_mut(&completed_task_id) {
                        *status = true;
                        println!(
                            "Task {}: Dependency {} completed",
                            task_id, completed_task_id
                        );
                    } else {
                        println!(
                            "Task {}: Received completion for non-dependency task {}",
                            task_id, completed_task_id
                        );
                    }
                    Ok(())
                }) as Pin<Box<dyn Future<Output = Result<()>> + Send + Sync>>
            }
        };

        let communicator = Arc::new(TaskCommunicator::new(
            config.id,
            address_map.clone(),
            dependency_handler,
        ));

        Task {
            config,
            status: Arc::new(Mutex::new(TaskStatus::Pending)),
            start_time: Arc::new(Mutex::new(None)),
            end_time: Arc::new(Mutex::new(None)),
            execution_lock: Arc::new(AtomicBool::new(true)),
            dependency_status: dependency_status,
            address_map,
            execution_order: Arc::new(Mutex::new(None)),
            communicator,
            dependent_tasks: Arc::new(dependent_tasks),
        }
    }

    pub async fn run(&self) -> Result<()> {
        // if !self.can_start().await {
        //     return Ok(());
        // }
        for attempt in 1..=MAX_RETRY_ATTEMPTS {
            if self.can_start().await {
                break;
            }
            if attempt == MAX_RETRY_ATTEMPTS {
                return Err(NightError::Task(format!(
                    "Task {} failed to start after {} attempts",
                    self.config.name, MAX_RETRY_ATTEMPTS
                )));
            }
            println!(
                "Task {}: Waiting for dependencies, attempt {}/{}",
                self.config.name, attempt, MAX_RETRY_ATTEMPTS
            );
            tokio::time::sleep(RETRY_DELAY).await;
        }

        let order = GLOBAL_EXECUTION_ORDER.fetch_add(1, Ordering::SeqCst);

        if let Ok(mut guard) = self.execution_order.lock() {
            *guard = Some(order);
        } else {
            // Handle the case where the mutex is poisoned
            println!(
                "Warning: Failed to set execution order for task {}",
                self.config.name
            );
        }

        // println!("Task: Starting execution of {}", self.config.name);
        self.set_status(TaskStatus::Running).await;

        // let result = self.run_once().await;
        let result = if self.config.is_periodic {
            self.run_periodic().await
        } else {
            self.run_once().await
        };

        match &result {
            Ok(_) => {
                // println!("Task: Successfully completed {}", self.config.name);
                if !self.config.is_periodic {
                    self.set_status(TaskStatus::Completed).await;
                    println!("Task: Successfully completed {}", self.config.name);
                    self.notify_completion().await?;
                }
            }
            Err(_e) => {
                // println!("Task: Failed to execute {}. Error: {:?}", self.config.name, e);
                self.set_status(TaskStatus::Failed).await;
            }
        }

        result
    }

    pub async fn get_execution_order(&self) -> Option<usize> {
        // Safely get the execution order
        self.execution_order.lock().ok().and_then(|guard| *guard)
    }

    pub async fn run_once(&self) -> Result<()> {
        // println!("Completed task: {}", self.config.name);
        self.set_status(TaskStatus::Completed).await;
        Ok(())
    }

    pub async fn run_periodic(&self) -> Result<()> {
        let mut interval = interval(self.parse_interval()?);

        loop {
            interval.tick().await;
            if !self.execution_lock.load(Ordering::Relaxed) {
                break;
            }
            self.run_once().await?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    async fn can_start(&self) -> bool {
        let dependencies = self.dependency_status.lock().unwrap();
        dependencies.values().all(|&status| status)
    }

    async fn set_status(&self, new_status: TaskStatus) {
        let mut status = self.status.lock().unwrap();
        *status = new_status;
    }

    async fn notify_completion(&self) -> Result<()> {
        for attempt in 1..=MAX_RETRY_ATTEMPTS {
            match self
                .communicator
                .notify_completion(self.dependent_tasks.clone())
                .await
            {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if attempt == MAX_RETRY_ATTEMPTS {
                        return Err(NightError::Communication(format!(
                            "Failed to notify completion after {} attempts: {}",
                            MAX_RETRY_ATTEMPTS, e
                        )));
                    }
                    println!(
                        "Task {}: Failed to notify completion, attempt {}/{}",
                        self.config.name, attempt, MAX_RETRY_ATTEMPTS
                    );
                    tokio::time::sleep(SEND_DELAY).await;
                }
            }
        }
        unreachable!()
    }

    pub async fn handle_dependency_completion(&self, completed_task_id: Uuid) -> Result<()> {
        let mut dependencies = self.dependency_status.lock().unwrap();
        if let Some(status) = dependencies.get_mut(&completed_task_id) {
            *status = true;
            println!(
                "Task {}: Dependency {} completed",
                self.config.name, completed_task_id
            );
        } else {
            println!(
                "Task {}: Received completion for non-dependency task {}",
                self.config.name, completed_task_id
            );
        }
        Ok(())
    }

    pub async fn start_listening(&self, port: u16) -> Result<()> {
        let task_id = self.config.id;
        let communicator = self.communicator.clone();

        tokio::spawn(async move {
            if let Err(e) = communicator.start_listener(port).await {
                eprintln!("Error in task {} listener: {:?}", task_id, e);
            }
        });

        Ok(())
    }

    fn parse_interval(&self) -> Result<Duration> {
        // Parse the interval string into a Duration
        // For simplicity, let's assume it's always in milliseconds
        self.config
            .interval
            .parse::<u64>()
            .map(Duration::from_millis)
            .map_err(|_| NightError::Task("Invalid interval format".to_string()))
    }

    pub fn get_info(&self) -> TaskInfo {
        TaskInfo {
            id: self.config.id,
            status: *self.status.lock().unwrap(),
            start_time: *self.start_time.lock().unwrap(),
            end_time: *self.end_time.lock().unwrap(),
            execution_order: *self.execution_order.lock().unwrap(),
        }
    }

    pub fn set_dependency_status(&self, dependency_id: Uuid, status: bool) {
        let mut dependencies = self.dependency_status.lock().unwrap();
        if let Some(dep_status) = dependencies.get_mut(&dependency_id) {
            *dep_status = status;
        }
    }

    pub fn set_execution_lock(&self, status: bool) {
        self.execution_lock.store(status, Ordering::Relaxed);
    }
}
