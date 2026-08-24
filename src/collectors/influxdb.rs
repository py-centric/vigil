use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::collectors::state::AppState;

#[derive(serde::Deserialize)]
struct OrgInfo {
    name: String,
}

#[derive(serde::Deserialize)]
struct OrgsResponse {
    orgs: Vec<OrgInfo>,
}

#[derive(serde::Deserialize)]
struct BucketInfo {
    name: String,
}

#[derive(serde::Deserialize)]
struct BucketsResponse {
    buckets: Vec<BucketInfo>,
}

pub fn start_influxdb_collector(uri: String, token: Option<String>, state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        loop {
            match fetch_all(&uri, &token, &state).await {
                Ok(_) => {
                    if let Ok(mut s) = state.lock() {
                        s.influxdb_online = true;
                    }
                }
                Err(e) => {
                    tracing::error!("InfluxDB collector error: {:?}", e);
                    if let Ok(mut s) = state.lock() {
                        s.influxdb_online = false;
                        let err_msg = format!("InfluxDB Error: {}", e);
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

async fn fetch_all(uri: &str, token: &Option<String>, state: &Arc<Mutex<AppState>>) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let auth_token = token.as_deref().unwrap_or("").trim();
    
    // Test connection with /ping or /health
    let health_url = format!("{}/health", uri);
    let health_resp = client.get(&health_url).send().await?;
    if !health_resp.status().is_success() {
        anyhow::bail!("InfluxDB /health returned {}", health_resp.status());
    }

    // Fetch Orgs
    let orgs_url = format!("{}/api/v2/orgs", uri);
    let mut org_names = Vec::new();
    let orgs_resp = client
        .get(&orgs_url)
        .header("Authorization", format!("Token {}", auth_token))
        .send()
        .await?;
        
    if orgs_resp.status().is_success() {
        if let Ok(data) = orgs_resp.json::<OrgsResponse>().await {
            for o in data.orgs {
                org_names.push(o.name);
            }
        }
    } else {
        anyhow::bail!("InfluxDB /api/v2/orgs returned {}", orgs_resp.status());
    }

    // Fetch Buckets
    let buckets_url = format!("{}/api/v2/buckets", uri);
    let mut bucket_names = Vec::new();
    let buckets_resp = client
        .get(&buckets_url)
        .header("Authorization", format!("Token {}", auth_token))
        .send()
        .await?;
        
    if buckets_resp.status().is_success() {
        if let Ok(data) = buckets_resp.json::<BucketsResponse>().await {
            for b in data.buckets {
                if !b.name.starts_with('_') { // skip internal
                    bucket_names.push(b.name);
                }
            }
        }
    } else {
        anyhow::bail!("InfluxDB /api/v2/buckets returned {}", buckets_resp.status());
    }

    if let Ok(mut s) = state.lock() {
        s.influxdb_orgs = org_names;
        s.influxdb_buckets = bucket_names;
    }

    Ok(())
}
