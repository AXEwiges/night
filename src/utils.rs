use crate::task::TaskId;
use rand::Rng;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Generate a unique task ID
pub fn generate_task_id() -> TaskId {
    let mut rng = rand::thread_rng();
    rng.gen()
}

// Implement consistent hashing for distributing tasks across nodes
pub struct ConsistentHash<T>
where
    T: Hash + Eq + std::fmt::Debug,
{
    nodes: Vec<T>,
    virtual_nodes: u32,
}

impl<T> ConsistentHash<T>
where
    T: Hash + Eq + std::fmt::Debug,
{
    pub fn new(nodes: Vec<T>, virtual_nodes: u32) -> Self {
        ConsistentHash {
            nodes,
            virtual_nodes,
        }
    }

    pub fn get_node(&self, key: &str) -> Option<&T> {
        if self.nodes.is_empty() {
            return None;
        }

        let hash = self.hash(key);
        let nodes_with_hash: Vec<(u64, &T)> = self
            .nodes
            .iter()
            .flat_map(|node| {
                (0..self.virtual_nodes).map(move |i| {
                    let virtual_node_key = format!("{}{}", self.hash(&format!("{:?}", node)), i);
                    (self.hash(&virtual_node_key), node)
                })
            })
            .collect();

        nodes_with_hash
            .iter()
            .find(|&&(node_hash, _)| node_hash >= hash)
            .or_else(|| nodes_with_hash.first())
            .map(|&(_, node)| node)
    }

    pub fn add_node(&mut self, node: T) {
        self.nodes.push(node);
    }

    pub fn remove_node(&mut self, node: &T) {
        if let Some(pos) = self.nodes.iter().position(|x| x == node) {
            self.nodes.remove(pos);
        }
    }

    fn hash(&self, key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

// Helper function to parse duration from string
pub fn parse_duration(duration_str: &str) -> Option<std::time::Duration> {
    let parts: Vec<&str> = duration_str.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let value: u64 = parts[0].parse().ok()?;
    let unit = parts[1].to_lowercase();

    match unit.as_str() {
        "ms" | "milliseconds" | "millisecond" => Some(std::time::Duration::from_millis(value)),
        "s" | "seconds" | "second" => Some(std::time::Duration::from_secs(value)),
        "m" | "minutes" | "minute" => Some(std::time::Duration::from_secs(value * 60)),
        "h" | "hours" | "hour" => Some(std::time::Duration::from_secs(value * 3600)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_task_id() {
        let id1 = generate_task_id();
        let id2 = generate_task_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_consistent_hash() {
        let nodes = vec!["node1", "node2", "node3"];
        let ch = ConsistentHash::new(nodes, 100);

        let node = ch.get_node("test_key");
        assert!(node.is_some());

        let node1 = ch.get_node("key1");
        let node2 = ch.get_node("key2");
        assert!(node1.is_some() && node2.is_some());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            parse_duration("100 ms"),
            Some(std::time::Duration::from_millis(100))
        );
        assert_eq!(
            parse_duration("5 seconds"),
            Some(std::time::Duration::from_secs(5))
        );
        assert_eq!(
            parse_duration("2 minutes"),
            Some(std::time::Duration::from_secs(120))
        );
        assert_eq!(
            parse_duration("1 hour"),
            Some(std::time::Duration::from_secs(3600))
        );
        assert_eq!(parse_duration("invalid"), None);
    }
}
