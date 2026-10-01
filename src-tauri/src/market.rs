use crate::domain::AppError;
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static ITEM_ID_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn is_auth_status(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

async fn market_error(context: &str, resp: reqwest::Response) -> AppError {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let body_hint = body.trim();
    let detail = if body_hint.is_empty() {
        format!("{} failed with status {}", context, status)
    } else {
        format!("{} failed with status {}: {}", context, status, body_hint)
    };

    if is_auth_status(status) {
        AppError::AuthExpired {
            message: "Your warframe.market session has expired. Please sign in again to continue."
                .to_string(),
        }
    } else {
        AppError::Other(detail)
    }
}

fn get_cached_item_id(item_slug: &str) -> Option<String> {
    ITEM_ID_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .get(item_slug)
        .cloned()
}

fn cache_item_id(item_slug: &str, item_id: &str) {
    ITEM_ID_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(item_slug.to_string(), item_id.to_string());
}

pub struct MarketClient {
    client: Client,
    base_url: String,
}

impl MarketClient {
    pub fn new(token: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "User-Agent",
            reqwest::header::HeaderValue::from_static("WarframeUtilityApp/1.0.0 (Tauri Rust)"),
        );
        headers.insert(
            "Accept",
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            "Content-Type",
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let auth_val = format!("Bearer {}", token);
        if let Ok(hv) = reqwest::header::HeaderValue::from_str(&auth_val) {
            headers.insert("Authorization", hv);
        }

        headers.insert("platform", reqwest::header::HeaderValue::from_static("pc"));
        headers.insert("language", reqwest::header::HeaderValue::from_static("en"));
        headers.insert(
            "Origin",
            reqwest::header::HeaderValue::from_static("https://warframe.market"),
        );
        headers.insert(
            "Referer",
            reqwest::header::HeaderValue::from_static("https://warframe.market/"),
        );

        let client = Client::builder().default_headers(headers).build().unwrap();

        Self {
            client,
            base_url: "https://api.warframe.market/v2".to_string(),
        }
    }

    pub async fn validate_token(&self) -> Result<String, AppError> {
        let url = format!("{}/me", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if resp.status().is_success() {
            let data: Value = resp.json().await?;
            let slug = data["data"]["slug"]
                .as_str()
                .unwrap_or("Unknown Tenno")
                .to_string();
            Ok(slug)
        } else {
            Err(market_error("Authentication validation", resp).await)
        }
    }

    pub async fn get_lowest_price(&self, item_slug: &str) -> Result<Option<i32>, AppError> {
        let url = format!("{}/orders/item/{}/top", self.base_url, item_slug);
        let resp = self.client.get(&url).send().await?;
        if resp.status().is_success() {
            let data: Value = resp.json().await?;
            if let Some(sell_orders) = data["data"]["sell"].as_array()
                && let Some(first_order) = sell_orders.first()
                && let Some(plat) = first_order["platinum"].as_i64()
            {
                return Ok(Some(plat as i32));
            }
            Ok(None)
        } else if is_auth_status(resp.status()) {
            Err(market_error("Fetching lowest price", resp).await)
        } else {
            Ok(None)
        }
    }

    pub async fn get_item_id(&self, item_slug: &str) -> Result<String, AppError> {
        if let Some(item_id) = get_cached_item_id(item_slug) {
            return Ok(item_id);
        }

        let url = format!("{}/items/{}", self.base_url, item_slug);
        let resp = self.client.get(&url).send().await?;
        if resp.status().is_success() {
            let data: Value = resp.json().await?;
            if let Some(id) = data["data"]["id"].as_str() {
                let item_id = id.to_string();
                cache_item_id(item_slug, &item_id);
                return Ok(item_id);
            }
        } else if is_auth_status(resp.status()) {
            return Err(market_error("Fetching item ID", resp).await);
        }
        Err(AppError::Other(format!(
            "Failed to retrieve item ID for '{}'",
            item_slug
        )))
    }

    pub async fn get_my_orders(&self) -> Result<HashMap<String, String>, AppError> {
        let url = format!("{}/orders/my", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let mut map = HashMap::new();
        if resp.status().is_success() {
            let data: Value = resp.json().await?;
            if let Some(orders) = data["data"].as_array() {
                for order in orders {
                    if let (Some(item_id), Some(order_id)) =
                        (order["itemId"].as_str(), order["id"].as_str())
                    {
                        map.insert(item_id.to_string(), order_id.to_string());
                    }
                }
            }
            Ok(map)
        } else {
            Err(market_error("Fetching active orders", resp).await)
        }
    }

    pub async fn post_or_update_listing(
        &self,
        item_slug: &str,
        quantity: i32,
        price: i32,
        active_orders: &HashMap<String, String>,
    ) -> Result<(), AppError> {
        let item_id = self.get_item_id(item_slug).await?;
        let active_order = active_orders.get(&item_id).cloned();

        if let Some(order_id) = active_order {
            let url = format!("{}/order/{}", self.base_url, order_id);
            let payload = serde_json::json!({
                "platinum": price,
                "quantity": quantity
            });
            let resp = self.client.patch(&url).json(&payload).send().await?;
            if !resp.status().is_success() {
                return Err(
                    market_error(&format!("Updating listing for {}", item_slug), resp).await,
                );
            }
        } else {
            let url = format!("{}/order", self.base_url);
            let payload = serde_json::json!({
                "itemId": item_id,
                "type": "sell",
                "platinum": price,
                "quantity": quantity,
                "rank": 0,
                "visible": true
            });
            let resp = self.client.post(&url).json(&payload).send().await?;
            if !resp.status().is_success() {
                return Err(
                    market_error(&format!("Creating listing for {}", item_slug), resp).await,
                );
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
        Ok(())
    }

    pub async fn delete_listing(
        &self,
        item_slug: &str,
        active_orders: &HashMap<String, String>,
    ) -> Result<(), AppError> {
        let item_id = self.get_item_id(item_slug).await?;
        let active_order = active_orders.get(&item_id).cloned();

        if let Some(order_id) = active_order {
            let url = format!("{}/order/{}", self.base_url, order_id);
            let resp = self.client.delete(&url).send().await?;
            if !resp.status().is_success() {
                return Err(
                    market_error(&format!("Deleting listing for {}", item_slug), resp).await,
                );
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
        }

        Ok(())
    }
}
