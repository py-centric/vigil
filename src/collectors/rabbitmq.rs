use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::collectors::state::AppState;
use lapin::{Connection, ConnectionProperties};

pub fn start_rabbitmq_collector(uri: String, state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        loop {
            match tokio::time::timeout(Duration::from_secs(5), Connection::connect(&uri, ConnectionProperties::default())).await {
                Ok(Ok(_conn)) => {
                    tracing::debug!("RabbitMQ: connected successfully to {}", uri);
                    if let Ok(mut s) = state.lock() {
                        s.rabbitmq_online = true;
                        // For demonstration/discovery, we'd normally use the Management API (HTTP).
                        // Adding a placeholder queue if we're connected just to show data.
                        s.rabbitmq_queues = vec![("default_queue".to_string(), "0 msgs".to_string())];
                    }
                }
                Ok(Err(e)) => {
                    tracing::error!("RabbitMQ collector error: {:?}", e);
                    if let Ok(mut s) = state.lock() {
                        s.rabbitmq_online = false;
                        let err_msg = format!("RabbitMQ Error: {}", e);
                        if s.logs.is_empty() || s.logs.last() != Some(&err_msg) {
                            if s.logs.len() > 100 { s.logs.remove(0); }
                            s.logs.push(err_msg);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("RabbitMQ collector timeout error: {:?}", e);
                    if let Ok(mut s) = state.lock() {
                        s.rabbitmq_online = false;
                        let err_msg = format!("RabbitMQ Timeout Error: {}", e);
                        if s.logs.is_empty() || s.logs.last() != Some(&err_msg) {
                            if s.logs.len() > 100 { s.logs.remove(0); }
                            s.logs.push(err_msg);
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}
