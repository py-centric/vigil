use std::sync::{Arc, Mutex};
use std::time::Duration;
use mongodb::{Client, options::ClientOptions};
use mongodb::bson::Document;
use futures::StreamExt;
use crate::collectors::state::AppState;

pub fn start_mongodb_collector(uri: String, state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        loop {
            match fetch_all(&uri, &state).await {
                Ok(_) => {
                    if let Ok(mut s) = state.lock() {
                        s.mongo_online = true;
                    }
                }
                Err(e) => {
                    tracing::error!("MongoDB collector error: {:?}", e);
                    if let Ok(mut s) = state.lock() {
                        s.mongo_online = false;
                        let err_msg = format!("Mongo Error: {}", e);
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
    tracing::debug!("MongoDB: connecting to {}", uri);
    let client_options = tokio::time::timeout(Duration::from_secs(5), ClientOptions::parse(uri))
        .await
        .map_err(|_| anyhow::anyhow!("Timeout parsing URI"))??;
    
    let client = Client::with_options(client_options)?;
    tracing::debug!("MongoDB: listing databases...");

    let databases = tokio::time::timeout(Duration::from_secs(5), client.list_database_names())
        .await
        .map_err(|_| anyhow::anyhow!("Timeout listing DBs"))??;
    
    tracing::debug!("MongoDB: found {} databases: {:?}", databases.len(), databases);
    
    if let Ok(mut s) = state.lock() {
        s.mongo_dbs = databases.clone();
        s.mongo_db_size = format!("{} DBs", databases.len());
    }

    for db_name in databases {
        let db = client.database(&db_name);
        
        let mut colls = vec![];
        if let Ok(collections_res) = tokio::time::timeout(Duration::from_secs(5), db.list_collection_names()).await {
            if let Ok(collections) = collections_res {
                colls = collections;
            }
        }
        
        if let Ok(mut s) = state.lock() {
            s.mongo_collections.insert(db_name.clone(), colls.clone());
        }

        for coll_name in colls.into_iter().take(5) {
            let coll = db.collection::<Document>(&coll_name);
            let mut docs = vec![];
            
            if let Ok(cursor_res) = tokio::time::timeout(Duration::from_secs(5), coll.find(mongodb::bson::doc! {})).await {
                if let Ok(mut cursor) = cursor_res {
                    let mut count = 0;
                    while let Some(doc_res) = cursor.next().await {
                        let doc_res: Result<Document, mongodb::error::Error> = doc_res;
                        if let Ok(d) = doc_res {
                            docs.push(d.to_string());
                            count += 1;
                            if count >= 20 { break; } 
                        } else {
                            break;
                        }
                    }
                }
            }
            
            if let Ok(mut s) = state.lock() {
                s.mongo_docs.insert((db_name.clone(), coll_name), docs);
            }
        }
    }

    Ok(())
}
