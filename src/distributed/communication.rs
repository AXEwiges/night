use crate::distributed::node::{Node, NodeId};
use crate::error::{NightError, NightResult};
use crate::task::{Task, TaskId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Heartbeat(NodeId),
    TaskAssignment(TaskId, Task),
    TaskStatusUpdate(TaskId, String), // String represents the status
    NodeJoin(NodeId),
    NodeLeave(NodeId),
}

pub struct Communication {
    node: Arc<Node>,
}

impl Communication {
    pub fn new(node: Arc<Node>) -> Self {
        Communication { node }
    }

    pub async fn start_server(&self) -> NightResult<()> {
        let addr = *self.node.addr();
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| NightError::NetworkError(format!("Failed to bind to address: {}", e)))?;

        println!("Node {} listening on: {}", self.node.id(), addr);

        loop {
            let (socket, _) = listener.accept().await.map_err(|e| {
                NightError::NetworkError(format!("Failed to accept connection: {}", e))
            })?;

            let node = Arc::clone(&self.node);
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(socket, node).await {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }

    pub fn get_node(&self) -> &Arc<Node> {
        &self.node
    }

    async fn handle_connection(mut socket: TcpStream, node: Arc<Node>) -> NightResult<()> {
        let mut buf = [0; 1024];
        let n = socket
            .read(&mut buf)
            .await
            .map_err(|e| NightError::NetworkError(format!("Failed to read from socket: {}", e)))?;

        let message: Message = serde_json::from_slice(&buf[..n])
            .map_err(|e| NightError::DeserializationError(e.to_string()))?;

        match message {
            Message::Heartbeat(node_id) => {
                println!("Received heartbeat from node: {}", node_id);
                // Update node status or respond as needed
            }
            Message::TaskAssignment(task_id, _task) => {
                println!("Received task assignment: {}", task_id);
                node.add_task(task_id).await?;
                // Handle task assignment logic
            }
            Message::TaskStatusUpdate(task_id, status) => {
                println!(
                    "Received task status update for task {}: {}",
                    task_id, status
                );
                // Update task status
            }
            Message::NodeJoin(node_id) => {
                println!("Node joined: {}", node_id);
                // Handle new node joining the cluster
            }
            Message::NodeLeave(node_id) => {
                println!("Node left: {}", node_id);
                // Handle node leaving the cluster
            }
        }

        Ok(())
    }

    pub async fn send_message(&self, to_addr: &str, message: Message) -> NightResult<()> {
        let mut stream = TcpStream::connect(to_addr).await.map_err(|e| {
            NightError::NetworkError(format!("Failed to connect to {}: {}", to_addr, e))
        })?;

        let serialized = serde_json::to_vec(&message)
            .map_err(|e| NightError::SerializationError(e.to_string()))?;

        stream
            .write_all(&serialized)
            .await
            .map_err(|e| NightError::NetworkError(format!("Failed to send message: {}", e)))?;

        Ok(())
    }

    pub async fn broadcast(&self, nodes: &[Arc<Node>], message: Message) -> NightResult<()> {
        for node in nodes {
            if node.id() != self.node.id() {
                self.send_message(&node.addr().to_string(), message.clone())
                    .await?;
            }
        }
        Ok(())
    }
}
