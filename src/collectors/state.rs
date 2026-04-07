use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct AppState {
    // MongoDB
    pub mongo_online: bool,
    pub mongo_dbs: Vec<String>,
    pub mongo_collections: HashMap<String, Vec<String>>,
    pub mongo_docs: HashMap<(String, String), Vec<String>>,
    pub mongo_db_size: String,

    // Redis
    pub redis_online: bool,
    pub redis_dbs: usize,
    pub redis_keys: HashMap<usize, Vec<(String, String)>>,
    pub redis_vals: HashMap<(usize, String), String>,
    pub redis_mem: String,

    // Redis Streams
    pub redis_streams: Vec<String>,
    pub redis_stream_entries: HashMap<String, Vec<String>>,

    // Kafka
    pub kafka_online: bool,
    pub kafka_topics: Vec<String>,
    pub kafka_messages: HashMap<String, Vec<String>>,

    // RabbitMQ
    pub rabbitmq_online: bool,
    pub rabbitmq_queues: Vec<(String, String)>,
    pub rabbitmq_messages: HashMap<String, Vec<String>>,

    // Logs
    pub logs: Vec<String>,
}
