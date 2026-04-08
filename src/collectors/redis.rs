use std::sync::{Arc, Mutex};
use std::time::Duration;
use redis::AsyncCommands;
use crate::collectors::state::AppState;

pub fn start_redis_collector(uri: String, state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        loop {
            match fetch_all(&uri, &state).await {
                Ok(_) => {
                    if let Ok(mut s) = state.lock() {
                        s.redis_online = true;
                    }
                }
                Err(e) => {
                    tracing::error!("Redis collector error: {:?}", e);
                    if let Ok(mut s) = state.lock() {
                        s.redis_online = false;
                        let err_msg = format!("Redis Error: {}", e);
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

async fn fetch_all(uri: &str, state: &Arc<Mutex<AppState>>) -> anyhow::Result<()> {
    tracing::debug!("Redis: connecting to {}", uri);
    let client = redis::Client::open(uri)?;
    let mut con = tokio::time::timeout(Duration::from_secs(5), client.get_multiplexed_tokio_connection())
        .await
        .map_err(|_| anyhow::anyhow!("Timeout connecting to Redis"))??;

    let info: String = redis::cmd("INFO").query_async(&mut con).await?;
    let mut memory_human = String::from("0KB");
    for line in info.lines() {
        if line.starts_with("used_memory_human:") {
            memory_human = line.trim_start_matches("used_memory_human:").to_string();
        }
    }
    
    let mut db_count = 16;
    let db_conf: Result<Vec<String>, _> = redis::cmd("CONFIG").arg("GET").arg("databases").query_async(&mut con).await;
    if let Ok(conf) = db_conf {
        if conf.len() == 2 {
            db_count = conf[1].parse::<usize>().unwrap_or(16);
        }
    }

    if let Ok(mut s) = state.lock() {
        s.redis_dbs = db_count;
        s.redis_mem = memory_human;
    }

    // Try to get active DBs
    let keyspace: String = redis::cmd("INFO").arg("keyspace").query_async(&mut con).await?;
    let mut active_dbs = vec![];
    for line in keyspace.lines() {
        if line.starts_with("db") {
            if let Some(num_str) = line.split(':').next().and_then(|s| s.strip_prefix("db")) {
                if let Ok(num) = num_str.parse::<usize>() {
                    active_dbs.push(num);
                }
            }
        }
    }
    
    // We fetch for all DBs requested, usually we just fetch active ones or 0-15
    for db_idx in 0..db_count {
        tracing::debug!("Redis: selecting DB {}", db_idx);
        let _ : redis::Value = redis::cmd("SELECT").arg(db_idx).query_async(&mut con).await?;
        
        let mut keys: Vec<String> = vec![];
        let scan_res: Result<(String, Vec<String>), _> = redis::cmd("SCAN").arg("0").arg("COUNT").arg("20").query_async(&mut con).await;
        if let Ok(iter) = scan_res {
            keys = iter.1;
        }
        let mut typed_keys = vec![];
        
        for key in keys.iter().take(20) {
            let key_type: String = redis::cmd("TYPE").arg(key as &str).query_async(&mut con).await?;
            typed_keys.push((key.clone(), key_type.clone()));
            
            let val_str = match key_type.as_str() {
                "string" => {
                    let v: String = con.get(key as &str).await.unwrap_or_else(|_| "".to_string());
                    v
                }
                "hash" => {
                    let v: std::collections::HashMap<String, String> = con.hgetall(key as &str).await.unwrap_or_default();
                    format!("{:?}", v)
                }
                "list" => {
                    let v: Vec<String> = con.lrange(key as &str, 0, 10).await.unwrap_or_default();
                    format!("{:?}", v)
                }
                "set" => {
                    let v: Vec<String> = con.smembers(key as &str).await.unwrap_or_default();
                    format!("{:?}", v)
                }
                "zset" => {
                    let v: Vec<(String, f64)> = con.zrange_withscores(key as &str, 0, 10).await.unwrap_or_default();
                    format!("{:?}", v)
                }
                _ => "(unsupported type)".to_string(),
            };
            
            if let Ok(mut s) = state.lock() {
                s.redis_vals.insert((db_idx, key.clone()), val_str);
            }
        }
        
        if let Ok(mut s) = state.lock() {
            s.redis_keys.insert(db_idx, typed_keys);
        }
    }

    Ok(())
}
