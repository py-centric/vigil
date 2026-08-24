use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::collectors::state::AppState;

fn mgmt_base_url(amqp_uri: &str) -> String {
    let host = amqp_uri
        .strip_prefix("amqp://")
        .and_then(|s| s.split(':').next())
        .unwrap_or("127.0.0.1");
    format!("http://{}:15672", host)
}

pub fn start_rabbitmq_collector(uri: String, state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        loop {
            match fetch_all(&uri, &state).await {
                Ok(_) => {
                    if let Ok(mut s) = state.lock() {
                        s.rabbitmq_online = true;
                    }
                }
                Err(e) => {
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
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

#[derive(serde::Deserialize)]
struct QueueInfo {
    name: String,
    messages: u64,
}

async fn fetch_all(uri: &str, state: &Arc<Mutex<AppState>>) -> anyhow::Result<()> {
    tracing::debug!("RabbitMQ: connecting to management API from {}", uri);
    let base = mgmt_base_url(uri);
    let url = format!("{}/api/queues", base);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let resp = client
        .get(&url)
        .basic_auth("guest", Some("guest"))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("RabbitMQ management API returned {}", resp.status());
    }

    let queues: Vec<QueueInfo> = resp.json().await?;

    let mut queue_list = Vec::new();
    let mut queue_names = Vec::new();
    for q in &queues {
        queue_list.push((q.name.clone(), format!("{} msgs", q.messages)));
        queue_names.push(q.name.clone());
    }

    if let Ok(mut s) = state.lock() {
        s.rabbitmq_queues = queue_list;
        // Keep only messages for queues that still exist
        s.rabbitmq_messages.retain(|k, _| queue_names.contains(k));
    }

    Ok(())
}
