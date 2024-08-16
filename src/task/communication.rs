use crate::common::types::{TaskAddressMap, TaskCommunication};
use crate::utils::error::{NightError, Result};
use serde_json;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

type AsyncDependencyHandler =
    dyn Fn(Uuid) -> Pin<Box<dyn Future<Output = Result<()>> + Send + Sync>> + Send + Sync;

pub struct TaskCommunicator {
    task_id: Uuid,
    address_map: Arc<TaskAddressMap>,
    dependency_handler: Arc<AsyncDependencyHandler>,
}

impl TaskCommunicator {
    pub fn new(
        task_id: Uuid,
        address_map: Arc<TaskAddressMap>,
        dependency_handler: impl Fn(Uuid) -> Pin<Box<dyn Future<Output = Result<()>> + Send + Sync>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        TaskCommunicator {
            task_id,
            address_map,
            dependency_handler: Arc::new(dependency_handler),
        }
    }

    pub async fn start_listener(&self, port: u16) -> Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| NightError::Communication(format!("Failed to bind to {}: {}", addr, e)))?;

        println!("Task {} listening on {}", self.task_id, addr);

        loop {
            let (socket, _) = listener.accept().await.map_err(|e| {
                NightError::Communication(format!("Failed to accept connection: {}", e))
            })?;

            let communicator = self.clone();
            tokio::spawn(async move {
                if let Err(e) = communicator.handle_connection(socket).await {
                    eprintln!("Error handling connection: {:?}", e);
                }
            });
        }
    }

    async fn handle_connection(&self, mut socket: TcpStream) -> Result<()> {
        let mut buffer = [0; 1024];
        let n = socket
            .read(&mut buffer)
            .await
            .map_err(|e| NightError::Communication(format!("Failed to read from socket: {}", e)))?;

        let message: TaskCommunication = serde_json::from_slice(&buffer[..n])
            .map_err(|e| NightError::Communication(format!("Failed to parse message: {}", e)))?;

        self.process_message(message).await?;

        Ok(())
    }

    pub async fn notify_completion(&self, dependent_tasks: Arc<Vec<Uuid>>) -> Result<()> {
        for &task_id in dependent_tasks.iter() {
            self.send_message(task_id, "COMPLETED".to_string(), self.task_id.to_string())
                .await?;
        }
        Ok(())
    }

    async fn process_message(&self, message: TaskCommunication) -> Result<()> {
        match message.message_type.as_str() {
            "COMPLETED" => {
                let completed_task_id = Uuid::parse_str(&message.payload).map_err(|e| {
                    NightError::Communication(format!("Invalid UUID in payload: {}", e))
                })?;

                println!(
                    "Task {} received completion notification from {}",
                    self.task_id, completed_task_id
                );

                let future = (self.dependency_handler)(completed_task_id);
                Pin::from(future).await?;
            }
            _ => println!("Unknown message type received: {}", message.message_type),
        }
        Ok(())
    }

    pub async fn send_message(
        &self,
        receiver_id: Uuid,
        message_type: String,
        payload: String,
    ) -> Result<()> {
        let receiver_address = self.address_map.get(&receiver_id).ok_or_else(|| {
            NightError::Communication(format!("Address not found for task {}", receiver_id))
        })?;

        let message = TaskCommunication {
            sender_id: self.task_id,
            receiver_id,
            message_type,
            payload,
        };

        let message_json = serde_json::to_string(&message).map_err(|e| {
            NightError::Communication(format!("Failed to serialize message: {}", e))
        })?;

        let mut stream = TcpStream::connect(receiver_address).await.map_err(|e| {
            NightError::Communication(format!("Failed to connect to {}: {}", receiver_address, e))
        })?;

        stream
            .write_all(message_json.as_bytes())
            .await
            .map_err(|e| NightError::Communication(format!("Failed to send message: {}", e)))?;

        Ok(())
    }
}

impl Clone for TaskCommunicator {
    fn clone(&self) -> Self {
        TaskCommunicator {
            task_id: self.task_id,
            address_map: Arc::clone(&self.address_map),
            dependency_handler: Arc::clone(&self.dependency_handler),
        }
    }
}
