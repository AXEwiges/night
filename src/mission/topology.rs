use crate::common::types::TaskConfig;
use crate::utils::error::{NightError, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

pub struct TopologyManager {
    tasks: HashMap<Uuid, TaskConfig>,
    dependencies: HashMap<Uuid, Vec<Uuid>>,
    reverse_dependencies: HashMap<Uuid, Vec<Uuid>>,
}

impl TopologyManager {
    pub fn new(tasks: Vec<TaskConfig>) -> Result<Self> {
        let mut topology = TopologyManager {
            tasks: HashMap::new(),
            dependencies: HashMap::new(),
            reverse_dependencies: HashMap::new(),
        };

        for task in tasks {
            topology.add_task(task)?;
        }

        topology.validate()?;

        Ok(topology)
    }

    fn add_task(&mut self, task: TaskConfig) -> Result<()> {
        let task_id = task.id;
        self.tasks.insert(task_id, task.clone());
        self.dependencies.insert(task_id, task.dependencies.clone());

        for dep_id in &task.dependencies {
            self.reverse_dependencies
                .entry(*dep_id)
                .or_insert_with(Vec::new)
                .push(task_id);
        }

        Ok(())
    }

    fn validate(&self) -> Result<()> {
        // Check for cycles
        if let Err(cycle) = self.detect_cycle() {
            return Err(NightError::Mission(format!(
                "Cyclic dependency detected: {:?}",
                cycle
            )));
        }

        // Check for missing dependencies
        for (task_id, deps) in &self.dependencies {
            for dep_id in deps {
                if !self.tasks.contains_key(dep_id) {
                    return Err(NightError::Mission(format!(
                        "Task {} depends on non-existent task {}",
                        task_id, dep_id
                    )));
                }
            }
        }

        Ok(())
    }

    fn detect_cycle(&self) -> Result<()> {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();

        for &task_id in self.tasks.keys() {
            if self.is_cyclic(task_id, &mut visited, &mut stack) {
                return Err(NightError::Mission(format!(
                    "Cyclic dependency detected: {:?}",
                    stack
                )));
            }
        }

        Ok(())
    }

    fn is_cyclic(
        &self,
        task_id: Uuid,
        visited: &mut HashSet<Uuid>,
        stack: &mut HashSet<Uuid>,
    ) -> bool {
        if stack.contains(&task_id) {
            return true;
        }

        if visited.contains(&task_id) {
            return false;
        }

        visited.insert(task_id);
        stack.insert(task_id);

        if let Some(deps) = self.dependencies.get(&task_id) {
            for &dep_id in deps {
                if self.is_cyclic(dep_id, visited, stack) {
                    return true;
                }
            }
        }

        stack.remove(&task_id);
        false
    }

    pub fn get_execution_order(&self) -> Vec<Vec<Uuid>> {
        let mut in_degree = HashMap::new();
        for &task_id in self.tasks.keys() {
            in_degree.insert(
                task_id,
                self.dependencies.get(&task_id).map_or(0, |deps| deps.len()),
            );
        }

        let mut queue = VecDeque::new();
        for (&task_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(task_id);
            }
        }

        let mut result = Vec::new();
        while !queue.is_empty() {
            let mut level = Vec::new();
            for _ in 0..queue.len() {
                let task_id = queue.pop_front().unwrap();
                level.push(task_id);

                if let Some(rev_deps) = self.reverse_dependencies.get(&task_id) {
                    for &dep_id in rev_deps {
                        let entry = in_degree.get_mut(&dep_id).unwrap();
                        *entry -= 1;
                        if *entry == 0 {
                            queue.push_back(dep_id);
                        }
                    }
                }
            }
            result.push(level);
        }

        result
    }

    pub fn get_task(&self, task_id: &Uuid) -> Option<&TaskConfig> {
        self.tasks.get(task_id)
    }

    pub fn get_dependencies(&self, task_id: &Uuid) -> Option<&Vec<Uuid>> {
        self.dependencies.get(task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_task(id: u128, deps: Vec<u128>) -> TaskConfig {
        TaskConfig {
            name: format!("Task {}", id),
            id: Uuid::from_u128(id),
            command: "echo test".to_string(),
            is_periodic: false,
            interval: "0".to_string(),
            importance: 1,
            dependencies: deps.into_iter().map(Uuid::from_u128).collect(),
        }
    }

    #[test]
    fn test_valid_topology() {
        let tasks = vec![
            create_test_task(1, vec![]),
            create_test_task(2, vec![1]),
            create_test_task(3, vec![1]),
            create_test_task(4, vec![2, 3]),
        ];

        let topology = TopologyManager::new(tasks).unwrap();
        let order = topology.get_execution_order();
        assert_eq!(order.len(), 3);
        assert_eq!(order[0], vec![Uuid::from_u128(1)]);
        assert!(order[1].contains(&Uuid::from_u128(2)) && order[1].contains(&Uuid::from_u128(3)));
        assert_eq!(order[2], vec![Uuid::from_u128(4)]);
    }

    #[test]
    fn test_cyclic_topology() {
        let tasks = vec![
            create_test_task(1, vec![3]),
            create_test_task(2, vec![1]),
            create_test_task(3, vec![2]),
        ];

        let result = TopologyManager::new(tasks);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_dependency() {
        let tasks = vec![
            create_test_task(1, vec![]),
            create_test_task(2, vec![3]), // Task 3 doesn't exist
        ];

        let result = TopologyManager::new(tasks);
        assert!(result.is_err());
    }
}
