use crate::common::types::MissionConfig;
use crate::utils::error::{NightError, Result};
use serde_json;
use std::fs;
use std::path::Path;

pub struct ConfigManager;

impl ConfigManager {
    pub fn load_config<P: AsRef<Path>>(path: P) -> Result<MissionConfig> {
        let config_str = fs::read_to_string(path)
            .map_err(|e| NightError::Config(format!("Failed to read config file: {}", e)))?;

        let config: MissionConfig = serde_json::from_str(&config_str)
            .map_err(|e| NightError::Config(format!("Failed to parse config: {}", e)))?;

        Self::validate_config(&config)?;

        Ok(config)
    }

    fn validate_config(config: &MissionConfig) -> Result<()> {
        // Check for unique task IDs
        let mut task_ids = std::collections::HashSet::new();
        for task in &config.tasks {
            if !task_ids.insert(task.id) {
                return Err(NightError::Config(format!(
                    "Duplicate task ID: {}",
                    task.id
                )));
            }
        }

        // Validate task dependencies
        for task in &config.tasks {
            for dep_id in &task.dependencies {
                if !task_ids.contains(dep_id) {
                    return Err(NightError::Config(format!(
                        "Task {} depends on non-existent task {}",
                        task.id, dep_id
                    )));
                }
            }
        }

        // Additional validations can be added here

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_valid_config() {
        let config_json = r#"
        {
            "name": "Test Mission",
            "tasks": [
                {
                    "name": "Task 1",
                    "id": "00000000-0000-0000-0000-000000000001",
                    "command": "echo Hello",
                    "is_periodic": false,
                    "interval": "",
                    "importance": 1,
                    "dependencies": []
                }
            ]
        }
        "#;

        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, config_json).unwrap();

        let result = ConfigManager::load_config(config_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_invalid_config() {
        let config_json = r#"
        {
            "name": "Invalid Mission",
            "tasks": [
                {
                    "name": "Task 1",
                    "id": "00000000-0000-0000-0000-000000000001",
                    "command": "echo Hello",
                    "is_periodic": false,
                    "interval": "",
                    "importance": 1,
                    "dependencies": ["00000000-0000-0000-0000-000000000002"]
                }
            ]
        }
        "#;

        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, config_json).unwrap();

        let result = ConfigManager::load_config(config_path);
        assert!(result.is_err());
    }
}
