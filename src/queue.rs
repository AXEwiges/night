use crate::task::{Priority, Task, TaskId};
use crossbeam::queue::SegQueue;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct PriorityQueue {
    queues: RwLock<BTreeMap<Priority, Arc<SegQueue<Arc<Task>>>>>,
}

impl PriorityQueue {
    pub fn new() -> Self {
        PriorityQueue {
            queues: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn push(&self, task: Arc<Task>) {
        let priority = task.priority();
        let mut queues = self.queues.write();
        let queue = queues
            .entry(priority)
            .or_insert_with(|| Arc::new(SegQueue::new()));
        queue.push(task);
    }

    pub fn pop(&self) -> Option<Arc<Task>> {
        let queues = self.queues.read();
        for (_, queue) in queues.iter().rev() {
            if let Some(task) = queue.pop() {
                return Some(task);
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        let queues = self.queues.read();
        queues.values().all(|queue| queue.is_empty())
    }
}

pub struct TaskQueue {
    queue: Arc<PriorityQueue>,
    task_lookup: Arc<SegQueue<(TaskId, Arc<Task>)>>,
}

impl TaskQueue {
    pub fn new() -> Self {
        TaskQueue {
            queue: Arc::new(PriorityQueue::new()),
            task_lookup: Arc::new(SegQueue::new()),
        }
    }

    pub fn push(&self, task: Task) {
        let task = Arc::new(task);
        self.queue.push(Arc::clone(&task));
        self.task_lookup.push((task.id(), Arc::clone(&task)));
    }

    pub fn pop(&self) -> Option<Arc<Task>> {
        self.queue.pop()
    }

    pub fn get_task(&self, id: TaskId) -> Option<Arc<Task>> {
        let temp_queue = SegQueue::new();
        let mut found_task = None;

        while let Some((task_id, task)) = self.task_lookup.pop() {
            if task_id == id {
                found_task = Some(Arc::clone(&task));
            }
            temp_queue.push((task_id, task));
        }

        while let Some(item) = temp_queue.pop() {
            self.task_lookup.push(item);
        }

        found_task
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
