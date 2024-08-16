use crate::common::types::{MissionConfig, TaskAddressMap, TaskConfig, TaskInfo};
use crate::mission::topology::TopologyManager;
use crate::task::task::Task;
use crate::utils::error::{NightError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct Mission {
    pub config: MissionConfig,
    pub topology: Arc<TopologyManager>,
    pub tasks: Arc<RwLock<HashMap<Uuid, Arc<Task>>>>,
    pub address_map: Arc<TaskAddressMap>,
}

impl Mission {
    pub async fn new(config: MissionConfig) -> Result<Self> {
        let topology = Arc::new(TopologyManager::new(config.tasks.clone())?);
        let address_map = Arc::new(Self::generate_address_map(&config.tasks)?);

        let mut dependent_tasks_map = HashMap::new();
        for task in &config.tasks {
            for &dep_id in &task.dependencies {
                dependent_tasks_map
                    .entry(dep_id)
                    .or_insert_with(Vec::new)
                    .push(task.id);
            }
        }

        let tasks = Arc::new(RwLock::new(HashMap::new()));
        for task_config in &config.tasks {
            let dependent_tasks = dependent_tasks_map
                .get(&task_config.id)
                .cloned()
                .unwrap_or_default();
            let task = Arc::new(Task::new(
                task_config.clone(),
                address_map.clone(),
                dependent_tasks,
            ));
            tasks.write().await.insert(task_config.id, task);
        }

        Ok(Mission {
            config,
            topology,
            tasks,
            address_map,
        })
    }

    fn generate_address_map(tasks: &[TaskConfig]) -> Result<TaskAddressMap> {
        tasks
            .iter()
            .enumerate()
            .map(|(i, task)| Ok((task.id, format!("127.0.0.1:{}", 12000 + i).parse().unwrap())))
            .collect()
    }

    pub async fn start(&self) -> Result<()> {
        // Start all tasks listening
        for (task_id, task) in self.tasks.read().await.iter() {
            let port = self
                .address_map
                .get(task_id)
                .ok_or_else(|| {
                    NightError::Mission(format!("No address found for task {}", task_id))
                })?
                .port();
            task.start_listening(port).await?;
        }

        let execution_order = self.topology.get_execution_order();

        for (level, tasks) in execution_order.iter().enumerate() {
            println!("Mission: Starting execution of level {}", level);
            let mut handles = vec![];

            for &task_id in tasks {
                let task = self
                    .tasks
                    .read()
                    .await
                    .get(&task_id)
                    .cloned()
                    .ok_or_else(|| NightError::Mission(format!("Task {} not found", task_id)))?;

                println!(
                    "Mission: Scheduling task {} ({})",
                    task.config.name, task_id
                );
                let handle = tokio::spawn(async move { task.run().await });

                handles.push(handle);
            }

            for handle in handles {
                handle
                    .await
                    .map_err(|e| NightError::Mission(format!("Task execution failed: {}", e)))??;
            }
        }

        Ok(())
    }

    pub async fn stop_task(&self, task_id: Uuid) -> Result<()> {
        let task = self
            .tasks
            .read()
            .await
            .get(&task_id)
            .cloned()
            .ok_or_else(|| NightError::Mission(format!("Task {} not found", task_id)))?;

        task.set_execution_lock(false);
        Ok(())
    }

    pub async fn get_task_info(&self, task_id: Uuid) -> Result<TaskInfo> {
        let task = self
            .tasks
            .read()
            .await
            .get(&task_id)
            .cloned()
            .ok_or_else(|| NightError::Mission(format!("Task {} not found", task_id)))?;

        Ok(task.get_info())
    }

    pub async fn get_all_task_info(&self) -> HashMap<Uuid, TaskInfo> {
        let tasks = self.tasks.read().await;
        tasks
            .iter()
            .map(|(&id, task)| (id, task.get_info()))
            .collect()
    }

    pub fn get_address_map(&self) -> Arc<TaskAddressMap> {
        self.address_map.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::{TaskConfig, TaskStatus};

    fn create_test_config() -> MissionConfig {
        MissionConfig {
            name: "Test Mission".to_string(),
            tasks: vec![
                TaskConfig {
                    name: "Task 1".to_string(),
                    id: Uuid::new_v4(),
                    command: "echo Task 1".to_string(),
                    is_periodic: false,
                    interval: "0".to_string(),
                    importance: 1,
                    dependencies: vec![],
                },
                TaskConfig {
                    name: "Task 2".to_string(),
                    id: Uuid::new_v4(),
                    command: "echo Task 2".to_string(),
                    is_periodic: false,
                    interval: "0".to_string(),
                    importance: 1,
                    dependencies: vec![],
                },
            ],
        }
    }

    #[tokio::test]
    async fn test_mission_creation() {
        let config = create_test_config();
        let mission = Mission::new(config).await;
        assert!(mission.is_ok());
    }

    #[tokio::test]
    async fn test_mission_execution() {
        let config = create_test_config();
        let mission = Mission::new(config).await.unwrap();
        let result = mission.start().await;
        assert!(result.is_ok());

        // Check if all tasks are completed
        let task_info = mission.get_all_task_info().await;
        for (_, info) in task_info {
            assert_eq!(info.status, TaskStatus::Completed);
        }
    }

    #[tokio::test]
    async fn test_stop_task() {
        let config = create_test_config();
        let mission = Mission::new(config).await.unwrap();

        let task_id = mission.config.tasks[0].id;
        let result = mission.stop_task(task_id).await;
        assert!(result.is_ok());

        let task_info = mission.get_task_info(task_id).await.unwrap();
        assert_eq!(task_info.status, TaskStatus::Pending);
    }
}
