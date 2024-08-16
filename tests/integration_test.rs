use night::common::types::{MissionConfig, TaskConfig, TaskStatus};
use night::{get_task_info, init, run, Mission, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;
use tokio;
use tokio::time::Duration;
use uuid::Uuid;

// Helper function to create a temporary config file
fn create_temp_config_file(config: &MissionConfig) -> Result<(TempDir, PathBuf)> {
    let temp_dir = tempfile::tempdir()?;
    let config_path = temp_dir.path().join("config.json");
    let config_json = serde_json::to_string_pretty(&config)?;
    fs::write(&config_path, config_json)?;
    Ok((temp_dir, config_path))
}

// Helper function to create a complex test configuration
fn create_complex_test_config() -> (MissionConfig, HashMap<char, Uuid>) {
    let mut task_ids = HashMap::new();
    let task_names = vec!['A', 'B', 'C', 'D', 'E'];

    for &name in &task_names {
        task_ids.insert(name, Uuid::new_v4());
    }

    let tasks = vec![
        TaskConfig {
            name: "Task A".to_string(),
            id: task_ids[&'A'],
            command: "echo Task A".to_string(),
            is_periodic: false,
            interval: "1".to_string(),
            importance: 1,
            dependencies: vec![],
        },
        TaskConfig {
            name: "Task B".to_string(),
            id: task_ids[&'B'],
            command: "echo Task B".to_string(),
            is_periodic: false,
            interval: "1".to_string(),
            importance: 1,
            dependencies: vec![task_ids[&'A']],
        },
        TaskConfig {
            name: "Task C".to_string(),
            id: task_ids[&'C'],
            command: "echo Task C".to_string(),
            is_periodic: false,
            interval: "1".to_string(),
            importance: 1,
            dependencies: vec![task_ids[&'B'], task_ids[&'E']],
        },
        TaskConfig {
            name: "Task D".to_string(),
            id: task_ids[&'D'],
            command: "echo Task D".to_string(),
            is_periodic: false,
            interval: "1".to_string(),
            importance: 1,
            dependencies: vec![task_ids[&'C']],
        },
        TaskConfig {
            name: "Task E".to_string(),
            id: task_ids[&'E'],
            command: "echo Task E".to_string(),
            is_periodic: false,
            interval: "1".to_string(),
            importance: 1,
            dependencies: vec![task_ids[&'A']],
        },
    ];

    (
        MissionConfig {
            name: "Complex Test Mission".to_string(),
            tasks,
        },
        task_ids,
    )
}

async fn wait_for_tasks_completion(
    mission: &Mission,
    task_ids: &HashMap<char, Uuid>,
    max_wait: Duration,
) -> Result<()> {
    let start = Instant::now();
    while start.elapsed() < max_wait {
        let mut all_completed = true;
        for (&task_char, &id) in task_ids {
            match get_task_info(mission, id).await {
                Ok(info) => {
                    log::debug!("Task {} ({}): status: {:?}", task_char, id, info.status);
                    if info.status != TaskStatus::Completed {
                        all_completed = false;
                        break;
                    }
                }
                Err(e) => {
                    log::error!(
                        "Failed to get info for task {} ({}): {:?}",
                        task_char,
                        id,
                        e
                    );
                    all_completed = false;
                    break;
                }
            }
        }

        if all_completed {
            return Ok(());
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Err(NightError::Mission("Timed out waiting for tasks to complete".to_string()))
    Ok(())
}

#[tokio::test]
async fn test_complex_mission_initialization() -> Result<()> {
    let (config, _) = create_complex_test_config();
    let (_temp_dir, config_path) = create_temp_config_file(&config)?;

    let mission = init(config_path.to_str().unwrap()).await?;
    assert_eq!(mission.config.name, "Complex Test Mission");
    assert_eq!(mission.config.tasks.len(), 5);
    Ok(())
}

#[tokio::test]
async fn test_complex_mission_execution() -> Result<()> {
    let (config, task_ids) = create_complex_test_config();
    let (_temp_dir, config_path) = create_temp_config_file(&config)?;

    let mission = init(config_path.to_str().unwrap()).await?;

    // Start the mission in a separate task
    let mission_clone = mission.clone();
    tokio::spawn(async move {
        run(&mission_clone).await.unwrap();
    });

    // Wait for all tasks to complete
    wait_for_tasks_completion(&mission, &task_ids, Duration::from_secs(10)).await?;

    // Check if all tasks are completed
    for (task_char, &task_id) in &task_ids {
        let info = get_task_info(&mission, task_id).await?;
        assert_eq!(
            info.status,
            TaskStatus::Completed,
            "Task {} did not complete",
            task_char
        );
    }

    // Get execution orders
    let a_order = get_task_info(&mission, task_ids[&'A'])
        .await?
        .execution_order
        .unwrap();
    let b_order = get_task_info(&mission, task_ids[&'B'])
        .await?
        .execution_order
        .unwrap();
    let c_order = get_task_info(&mission, task_ids[&'C'])
        .await?
        .execution_order
        .unwrap();
    let d_order = get_task_info(&mission, task_ids[&'D'])
        .await?
        .execution_order
        .unwrap();
    let e_order = get_task_info(&mission, task_ids[&'E'])
        .await?
        .execution_order
        .unwrap();

    // Check execution order
    assert!(a_order < b_order, "Task A should execute before Task B");
    assert!(a_order < e_order, "Task A should execute before Task E");
    assert!(b_order < c_order, "Task B should execute before Task C");
    assert!(e_order < c_order, "Task E should execute before Task C");
    assert!(c_order < d_order, "Task C should execute before Task D");

    Ok(())
}
