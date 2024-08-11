pub mod communication;
pub mod node;

use self::communication::{Communication, Message};
use self::node::{Node, NodeId};
use crate::error::NightResult;
use crate::utils::ConsistentHash;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct DistributedSystem {
    nodes: RwLock<HashMap<NodeId, Arc<Node>>>,
    communication: Arc<Communication>,
    consistent_hash: RwLock<ConsistentHash<NodeId>>,
}

impl DistributedSystem {
    pub fn new(local_node: Arc<Node>) -> Self {
        let communication = Arc::new(Communication::new(Arc::clone(&local_node)));
        let mut nodes = HashMap::new();
        nodes.insert(local_node.id().clone(), Arc::clone(&local_node));

        DistributedSystem {
            nodes: RwLock::new(nodes),
            communication,
            consistent_hash: RwLock::new(ConsistentHash::new(vec![local_node.id().clone()], 100)),
        }
    }

    pub async fn add_node(&self, node: Arc<Node>) -> NightResult<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.id().clone(), Arc::clone(&node));

        let mut ch = self.consistent_hash.write().await;
        ch.add_node(node.id().clone());

        self.communication
            .broadcast(
                &nodes.values().cloned().collect::<Vec<_>>(),
                Message::NodeJoin(node.id().clone()),
            )
            .await?;
        Ok(())
    }

    pub async fn remove_node(&self, node_id: &NodeId) -> NightResult<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(node_id);

        let mut ch = self.consistent_hash.write().await;
        ch.remove_node(node_id);

        self.communication
            .broadcast(
                &nodes.values().cloned().collect::<Vec<_>>(),
                Message::NodeLeave(node_id.clone()),
            )
            .await?;
        Ok(())
    }

    pub async fn get_node_for_task(&self, task_id: &str) -> Option<Arc<Node>> {
        let ch = self.consistent_hash.read().await;
        let node_id = ch.get_node(task_id)?;
        self.nodes.read().await.get(node_id).cloned()
    }

    pub async fn start_server(&self) -> NightResult<()> {
        self.communication.start_server().await
    }

    pub async fn send_heartbeat(&self) -> NightResult<()> {
        let nodes = self.nodes.read().await;
        let local_id = self.communication.get_node().id().clone();
        self.communication
            .broadcast(
                &nodes.values().cloned().collect::<Vec<_>>(),
                Message::Heartbeat(local_id),
            )
            .await
    }
}
