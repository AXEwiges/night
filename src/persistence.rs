use crate::error::{NightError, NightResult};
use crate::task::{Task, TaskId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Serialize, Deserialize)]
struct PersistentState {
    tasks: HashMap<TaskId, Task>,
}

pub struct Persistence {
    file_path: String,
}

impl Persistence {
    pub fn new(file_path: String) -> Self {
        Persistence { file_path }
    }

    pub async fn save_state(&self, tasks: HashMap<TaskId, Task>) -> NightResult<()> {
        let state = PersistentState { tasks };
        let serialized = serde_json::to_string(&state)
            .map_err(|e| NightError::SerializationError(e.to_string()))?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.file_path)
            .await
            .map_err(|e| NightError::PersistenceError(e.to_string()))?;

        file.write_all(serialized.as_bytes())
            .await
            .map_err(|e| NightError::PersistenceError(e.to_string()))?;

        Ok(())
    }

    pub async fn load_state(&self) -> NightResult<HashMap<TaskId, Task>> {
        if !Path::new(&self.file_path).exists() {
            return Ok(HashMap::new());
        }

        let mut file = File::open(&self.file_path)
            .await
            .map_err(|e| NightError::PersistenceError(e.to_string()))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .await
            .map_err(|e| NightError::PersistenceError(e.to_string()))?;

        let state: PersistentState = serde_json::from_str(&contents)
            .map_err(|e| NightError::DeserializationError(e.to_string()))?;

        Ok(state.tasks)
    }
}
