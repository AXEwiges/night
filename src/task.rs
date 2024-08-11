use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub type TaskId = u64;
pub type Priority = u8;

pub trait Runnable: Send + Sync {
    fn run(&self) -> Result<(), Box<dyn std::error::Error>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Queued => write!(f, "Queued"),
            TaskStatus::Running => write!(f, "Running"),
            TaskStatus::Completed => write!(f, "Completed"),
            TaskStatus::Failed => write!(f, "Failed"),
            TaskStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    id: TaskId,
    priority: Priority,
    data: Arc<dyn Any + Send + Sync>,
    created_at: Instant,
    run_after: Option<Instant>,
    timeout: Option<Duration>,
    max_retries: u32,
    retries: u32,
    dependencies: Vec<TaskId>,
    pub status: TaskStatus,
}

impl Serialize for Task {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Task", 10)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("priority", &self.priority)?;
        state.serialize_field("created_at", &self.created_at.elapsed().as_secs())?;
        state.serialize_field("run_after", &self.run_after.map(|t| t.elapsed().as_secs()))?;
        state.serialize_field("timeout", &self.timeout.map(|d| d.as_secs()))?;
        state.serialize_field("max_retries", &self.max_retries)?;
        state.serialize_field("retries", &self.retries)?;
        state.serialize_field("dependencies", &self.dependencies)?;
        state.serialize_field("status", &self.status)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Task {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Id,
            Priority,
            CreatedAt,
            RunAfter,
            Timeout,
            MaxRetries,
            Retries,
            Dependencies,
            Status,
        }

        struct TaskVisitor;

        impl<'de> serde::de::Visitor<'de> for TaskVisitor {
            type Value = Task;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Task")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Task, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut id = None;
                let mut priority = None;
                let mut created_at = None;
                let mut run_after = None;
                let mut timeout = None;
                let mut max_retries = None;
                let mut retries = None;
                let mut dependencies = None;
                let mut status = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Id => id = Some(map.next_value()?),
                        Field::Priority => priority = Some(map.next_value()?),
                        Field::CreatedAt => created_at = Some(map.next_value()?),
                        Field::RunAfter => run_after = Some(map.next_value()?),
                        Field::Timeout => timeout = Some(map.next_value()?),
                        Field::MaxRetries => max_retries = Some(map.next_value()?),
                        Field::Retries => retries = Some(map.next_value()?),
                        Field::Dependencies => dependencies = Some(map.next_value()?),
                        Field::Status => status = Some(map.next_value()?),
                    }
                }

                let id = id.ok_or_else(|| serde::de::Error::missing_field("id"))?;
                let priority =
                    priority.ok_or_else(|| serde::de::Error::missing_field("priority"))?;
                let created_at = created_at
                    .map(|secs| Instant::now() - Duration::from_secs(secs))
                    .unwrap_or_else(Instant::now);
                let run_after = run_after
                    .map(|secs| Some(Instant::now() - Duration::from_secs(secs)))
                    .unwrap_or(None);
                let timeout = timeout
                    .map(|secs| Some(Duration::from_secs(secs)))
                    .unwrap_or(None);
                let max_retries =
                    max_retries.ok_or_else(|| serde::de::Error::missing_field("max_retries"))?;
                let retries = retries.ok_or_else(|| serde::de::Error::missing_field("retries"))?;
                let dependencies =
                    dependencies.ok_or_else(|| serde::de::Error::missing_field("dependencies"))?;
                let status = status.ok_or_else(|| serde::de::Error::missing_field("status"))?;

                Ok(Task {
                    id,
                    priority,
                    data: Arc::new(()),
                    created_at,
                    run_after,
                    timeout,
                    max_retries,
                    retries,
                    dependencies,
                    status,
                })
            }
        }

        const FIELDS: &[&str] = &[
            "id",
            "priority",
            "created_at",
            "run_after",
            "timeout",
            "max_retries",
            "retries",
            "dependencies",
            "status",
        ];
        deserializer.deserialize_struct("Task", FIELDS, TaskVisitor)
    }
}

impl Task {
    pub fn new(
        id: TaskId,
        priority: Priority,
        data: Arc<dyn Any + Send + Sync>,
        max_retries: u32,
        timeout: Option<Duration>,
        dependencies: Vec<TaskId>,
    ) -> Self {
        Task {
            id,
            priority,
            data,
            created_at: Instant::now(),
            run_after: None,
            timeout,
            max_retries,
            retries: 0,
            dependencies,
            status: TaskStatus::Queued, // 初始状态为 Queued
        }
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
    }

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn priority(&self) -> Priority {
        self.priority
    }

    pub fn data(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.data
    }

    pub fn increment_retries(&mut self) {
        self.retries += 1;
    }

    pub fn set_run_after(&mut self, run_after: Instant) {
        self.run_after = Some(run_after);
    }

    pub fn is_ready(&self) -> bool {
        self.run_after.map_or(true, |time| time <= Instant::now())
    }

    pub fn has_timed_out(&self) -> bool {
        self.timeout
            .map_or(false, |timeout| self.created_at.elapsed() > timeout)
    }

    pub fn can_retry(&self) -> bool {
        self.retries < self.max_retries
    }

    pub fn dependencies(&self) -> &[TaskId] {
        &self.dependencies
    }
}
