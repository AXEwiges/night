use crate::error::NightResult;
use crate::queue::TaskQueue;
use crate::task::{Task, TaskId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Scheduler {
    queue: Arc<TaskQueue>,
    dependency_graph: Mutex<HashMap<TaskId, HashSet<TaskId>>>,
    completed_tasks: Mutex<HashSet<TaskId>>,
}

impl Scheduler {
    pub fn new(queue: Arc<TaskQueue>) -> Self {
        Scheduler {
            queue,
            dependency_graph: Mutex::new(HashMap::new()),
            completed_tasks: Mutex::new(HashSet::new()),
        }
    }

    // Add a task to the scheduler
    pub async fn add_task(&self, task: Task) -> NightResult<()> {
        let task_id = task.id();
        let dependencies = task.dependencies().to_vec();

        // Update dependency graph
        let mut graph = self.dependency_graph.lock().await;
        for dep_id in &dependencies {
            graph.entry(*dep_id).or_default().insert(task_id);
        }

        // Add task to queue
        self.queue.push(task);

        Ok(())
    }

    // Get the next available task
    pub async fn next_task(&self) -> NightResult<Option<Arc<Task>>> {
        loop {
            if let Some(task) = self.queue.pop() {
                // let task_id = task.id();
                let dependencies = task.dependencies();

                // Check if all dependencies are completed
                let completed = self.completed_tasks.lock().await;
                if dependencies.iter().all(|dep| completed.contains(dep)) {
                    return Ok(Some(task));
                } else {
                    // Put the task back in the queue
                    self.queue.push((*task).clone());
                }
            } else {
                return Ok(None);
            }
        }
    }

    // Mark a task as completed
    pub async fn complete_task(&self, task_id: TaskId) -> NightResult<()> {
        let mut completed = self.completed_tasks.lock().await;
        completed.insert(task_id);

        // Check if any waiting tasks can now be executed
        let mut graph = self.dependency_graph.lock().await;
        if let Some(waiting_tasks) = graph.remove(&task_id) {
            for waiting_task_id in waiting_tasks {
                if let Some(task) = self.queue.get_task(waiting_task_id) {
                    if task
                        .dependencies()
                        .iter()
                        .all(|dep| completed.contains(dep))
                    {
                        // All dependencies are now completed, move the task to the front of the queue
                        self.queue.push((*task).clone());
                    }
                }
            }
        }

        Ok(())
    }
}
