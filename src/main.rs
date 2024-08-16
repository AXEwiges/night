use night::{get_all_task_info, get_task_info, init, run, stop_task, Result};
use std::env;
use tokio;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    night::utils::logging::init_logger()?;

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <config_file> [command] [task_id]", args[0]);
        std::process::exit(1);
    }

    let config_path = &args[1];
    let command = args.get(2).map(String::as_str);

    // Initialize the mission
    let mission = init(config_path).await?;

    match command {
        Some("run") => {
            log::info!("Starting mission execution");
            run(&mission).await?;
            log::info!("Mission execution completed");
        }
        Some("stop") => {
            if let Some(task_id) = args.get(3) {
                let uuid = Uuid::parse_str(task_id)
                    .map_err(|e| night::NightError::Mission(e.to_string()))?;
                log::info!("Stopping task: {}", uuid);
                stop_task(&mission, uuid).await?;
                log::info!("Task stopped successfully");
            } else {
                eprintln!("Error: Task ID is required for the stop command");
                std::process::exit(1);
            }
        }
        Some("info") => {
            if let Some(task_id) = args.get(3) {
                let uuid = Uuid::parse_str(task_id)
                    .map_err(|e| night::NightError::Mission(e.to_string()))?;
                let info = get_task_info(&mission, uuid).await?;
                println!("Task info: {:?}", info);
            } else {
                let all_info = get_all_task_info(&mission).await;
                println!("All task info: {:?}", all_info);
            }
        }
        _ => {
            eprintln!("Error: Unknown command. Available commands: run, stop, info");
            std::process::exit(1);
        }
    }

    Ok(())
}
