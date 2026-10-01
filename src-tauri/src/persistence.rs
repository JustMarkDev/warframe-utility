use crate::domain::{AppError, SyndicateState};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use tokio::fs;
use tokio::sync::Mutex;

static FILE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn get_file_lock() -> &'static Mutex<()> {
    FILE_LOCK.get_or_init(|| Mutex::new(()))
}

pub async fn load_all_standings<P: AsRef<Path>>(
    path: P,
) -> Result<HashMap<String, SyndicateState>, AppError> {
    let _lock = get_file_lock().lock().await;
    let factions = vec![
        "steel_meridian",
        "arbiters_of_hexis",
        "cephalon_suda",
        "perrin_sequence",
        "red_veil",
        "new_loka",
    ];

    if !path.as_ref().exists() {
        let mut default_states = HashMap::new();
        for f in factions {
            default_states.insert(f.to_string(), SyndicateState::new(f, 0, 0));
        }
        return Ok(default_states);
    }

    let content = match fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return Ok(HashMap::new()),
    };

    let raw: HashMap<String, serde_json::Value> =
        serde_json::from_str(&content).unwrap_or_else(|_| HashMap::new());

    let mut states = HashMap::new();
    for f in factions {
        let (standing, rank) = if let Some(val) = raw.get(f) {
            let standing = val.get("standing").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let rank = val.get("rank").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            (standing, rank)
        } else {
            (0, 0)
        };
        states.insert(f.to_string(), SyndicateState::new(f, standing, rank));
    }

    Ok(states)
}

pub async fn save_all_standings<P: AsRef<Path>>(
    path: P,
    states: &HashMap<String, SyndicateState>,
) -> Result<(), AppError> {
    let _lock = get_file_lock().lock().await;
    let mut raw = serde_json::Map::new();
    for (k, v) in states {
        let mut inner = serde_json::Map::new();
        inner.insert("standing".to_string(), serde_json::Value::from(v.standing));
        inner.insert("rank".to_string(), serde_json::Value::from(v.rank));
        raw.insert(k.clone(), serde_json::Value::Object(inner));
    }

    let content = serde_json::to_string_pretty(&serde_json::Value::Object(raw))
        .map_err(|e| AppError::Other(e.to_string()))?;
    fs::write(path, content).await?;
    Ok(())
}

pub async fn load_jwt<P: AsRef<Path>>(path: P) -> Result<Option<String>, AppError> {
    let _lock = get_file_lock().lock().await;
    if !path.as_ref().exists() {
        // Initialize file if not exist
        let _ = fs::write(&path, "").await;
        return Ok(None);
    }
    let content = fs::read_to_string(path).await?;
    let token = content.trim().to_string();
    if token.is_empty() || token == "PASTE_YOUR_WARFRAME_MARKET_JWT_COOKIE_HERE" {
        Ok(None)
    } else {
        Ok(Some(token))
    }
}

pub async fn save_jwt<P: AsRef<Path>>(path: P, token: &str) -> Result<(), AppError> {
    let _lock = get_file_lock().lock().await;
    fs::write(path, token.trim()).await?;
    Ok(())
}
