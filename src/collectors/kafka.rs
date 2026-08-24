use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::collectors::state::AppState;
use rskafka::client::{ClientBuilder, partition::OffsetAt};

pub fn start_kafka_collector(uri: String, state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        loop {
            match fetch_all(&uri, &state).await {
                Ok(_) => {
                    if let Ok(mut s) = state.lock() {
                        s.kafka_online = true;
                    }
                }
                Err(e) => {
                    tracing::error!("Kafka collector error: {:?}", e);
                    if let Ok(mut s) = state.lock() {
                        s.kafka_online = false;
                        let err_msg = format!("Kafka Error: {}", e);
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

async fn fetch_all(brokers: &str, state: &Arc<Mutex<AppState>>) -> anyhow::Result<()> {
    tracing::debug!("Kafka: connecting to brokers: {}", brokers);
    let client_builder = ClientBuilder::new(vec![brokers.to_string()]);
    let client = tokio::time::timeout(Duration::from_secs(5), client_builder.build())
        .await
        .map_err(|_| anyhow::anyhow!("Timeout building kafka client"))??;
    
    tracing::debug!("Kafka: listing topics...");
    let topics = client.list_topics().await?;
    let mut topic_names = vec![];
    for topic in &topics {
        if !topic.name.starts_with("__") { // skip internal
            topic_names.push(topic.name.clone());
        }
    }

    if let Ok(mut s) = state.lock() {
        s.kafka_topics = topic_names.clone();
    }

    for topic_name in topic_names.into_iter().take(5) {
        use rskafka::client::partition::UnknownTopicHandling;
        if let Ok(partition_client) = client.partition_client(&topic_name, 0, UnknownTopicHandling::Retry).await {
            if let Ok(watermark) = partition_client.get_offset(OffsetAt::Latest).await {
                if watermark > 0 {
                    let start = if watermark > 10 { watermark - 10 } else { 0 };
                    if let Ok(records) = partition_client.fetch_records(start, 1..100000, 10_000).await {
                        let mut msgs = vec![];
                        for record_and_offset in records.0 {
                            if let Some(val) = record_and_offset.record.value {
                                msgs.push(String::from_utf8_lossy(&val).to_string());
                            }
                        }
                        if let Ok(mut s) = state.lock() {
                            s.kafka_messages.insert(topic_name.clone(), msgs);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
