use crate::domain::{AppError, SyndicateState, get_syndicate_cost, get_syndicate_mods};
use crate::market::MarketClient;
use crate::persistence::{load_all_standings, load_jwt, save_all_standings, save_jwt};
use serde::Serialize;

const STATUS_PATH: &str = "syndicate_status.json";
const SETTINGS_PATH: &str = "settings.conf";

/// Syncs warframe.market sell listings for one faction to match the current
/// standing. If the faction has listable quantity > 0, every mod is posted or
/// undercut by 1 platinum. If standing has dropped to zero, every listing is
/// deleted. All market errors propagate — callers are responsible for deciding
/// whether to roll back any prior state changes.
async fn sync_listings(client: &MarketClient, state: &SyndicateState) -> Result<(), AppError> {
    let mods = get_syndicate_mods(&state.faction_key, state.rank);
    let qty = state.listable_quantity();

    let active_orders = client.get_my_orders().await?;

    if qty > 0 {
        for m in &mods {
            let lowest = client.get_lowest_price(m).await?;
            let price = match lowest {
                Some(p) => i32::max(1, p - 1),
                None => 10,
            };
            client
                .post_or_update_listing(m, qty, price, &active_orders)
                .await?;
        }
    } else {
        for m in &mods {
            client.delete_listing(m, &active_orders).await?;
        }
    }

    Ok(())
}

#[derive(Serialize)]
pub struct InitData {
    pub standings: Vec<SyndicateState>,
    pub authenticated: bool,
    pub account_name: Option<String>,
    pub auth_refresh_required: bool,
}

#[tauri::command]
pub async fn load_syndicates() -> Result<InitData, AppError> {
    let standings_map = load_all_standings(STATUS_PATH).await?;
    let mut standings: Vec<SyndicateState> = standings_map.into_values().collect();
    // Sort for stable UI layout
    standings.sort_by(|a, b| a.faction_key.cmp(&b.faction_key));

    let token_opt = load_jwt(SETTINGS_PATH).await?;
    let mut authenticated = false;
    let mut account_name = None;
    let mut auth_refresh_required = false;

    if let Some(token) = token_opt {
        let client = MarketClient::new(&token);
        match client.validate_token().await {
            Ok(slug) => {
                authenticated = true;
                account_name = Some(slug);
            }
            Err(AppError::AuthExpired { .. }) => {
                auth_refresh_required = true;
            }
            Err(_) => {}
        }
    }

    Ok(InitData {
        standings,
        authenticated,
        account_name,
        auth_refresh_required,
    })
}

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
pub async fn start_in_app_login(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    // Close any existing login window to avoid duplicate errors
    if let Some(existing) = app_handle.get_webview_window("market_login_window") {
        let _ = existing.close();
    }

    // 1. Preload script: keep it for debugging and console logs, but we will do
    //    the real cookie extraction via Tauri's native Rust Cookie API because
    //    the JWT cookie is marked as HttpOnly & Secure and cannot be accessed via JS document.cookie.
    let preload_script = r#"
        (function() {
          console.log('[WFU-Auth] Preload script injected. Waiting for JWT cookie... Note: JWT is HttpOnly and Secure, so the Rust background task will extract it.');
        })();
    "#;

    // 2. Open the official login page in a dedicated WebView window.
    //    IMPORTANT: Use WebviewUrl::External for off-app URLs.
    //    WebviewUrl::App is only for local frontend routes (e.g. "/login").
    let url: tauri::Url = "https://warframe.market/auth/signin"
        .parse()
        .map_err(|e| AppError::Other(format!("Invalid login URL: {}", e)))?;

    let _window = WebviewWindowBuilder::new(
        &app_handle,
        "market_login_window",
        WebviewUrl::External(url),
    )
    .title("Warframe.market – Secure Sign In")
    .inner_size(700.0, 800.0)
    .resizable(true)
    .initialization_script(preload_script)
    .build()
    .map_err(|e| AppError::Other(format!("Failed to open login window: {}", e)))?;

    println!("[WFU-Auth] Login window opened successfully. Starting Rust cookie poller...");

    // 3. Spawn a background Tokio task to poll window.cookies().
    //    This is more robust than cookies_for_url because it queries the entire cookie store
    //    regardless of URL/scheme/domain matching.
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        println!("[WFU-Auth] Started background cookie polling task in Rust.");

        let mut attempts = 0;
        let mut last_attempted_token = String::new();

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            attempts += 1;
            if attempts > 600 {
                // 5 minutes timeout
                println!("[WFU-Auth] Rust cookie polling timed out after 5 minutes.");
                break;
            }

            // Retrieve the window
            let window = match app_handle_clone.get_webview_window("market_login_window") {
                Some(w) => w,
                None => {
                    // Window was closed by user
                    println!("[WFU-Auth] Login window closed. Stopping Rust cookie poll.");
                    break;
                }
            };

            // Retrieve all cookies currently in the jar
            match window.cookies() {
                Ok(cookies) => {
                    let mut found_token = None;
                    let mut cookie_names = Vec::new();

                    for cookie in &cookies {
                        let name = cookie.name().to_string();
                        let domain = cookie
                            .domain()
                            .map(|d| d.to_string())
                            .unwrap_or_else(|| "none".to_string());
                        cookie_names.push(format!("{} ({})", name, domain));

                        if name == "JWT" {
                            found_token = Some(cookie.value().to_string());
                        }
                    }

                    // Print the list of cookies every 5 seconds (10 attempts) so the user can debug
                    if attempts % 10 == 0 {
                        println!(
                            "[WFU-Auth] Active cookies in jar (attempt {}): {:?}",
                            attempts, cookie_names
                        );
                    }

                    if let Some(token) = found_token
                        && token != last_attempted_token
                    {
                        last_attempted_token = token.clone();
                        println!(
                            "[WFU-Auth] Success! New JWT cookie captured via native cookies() API. Validating..."
                        );

                        // Call capture_market_jwt to validate, save, close window, and emit auth_success
                        match capture_market_jwt(app_handle_clone.clone(), token).await {
                            Ok(slug) => {
                                println!("[WFU-Auth] Login complete! Account: {}", slug);
                                break;
                            }
                            Err(e) => {
                                eprintln!(
                                    "[WFU-Auth] Error during validation/capture for this token: {:?}",
                                    e
                                );
                                // Do NOT break the loop! A temporary, placeholder, or old expired token
                                // shouldn't kill the poller. We continue waiting for a new/valid token.
                            }
                        }
                    }
                }
                Err(e) => {
                    if attempts % 20 == 0 {
                        println!(
                            "[WFU-Auth] window.cookies() returned error: {}. Still trying...",
                            e
                        );
                    }
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn capture_market_jwt(
    app_handle: tauri::AppHandle,
    token: String,
) -> Result<String, AppError> {
    let client = MarketClient::new(&token);

    // 1. Validate the captured token with a request to warframe.market/v2/me
    let slug = client.validate_token().await?;

    // 2. Save token to user settings file
    save_jwt(SETTINGS_PATH, &token).await?;

    // 3. Find and close the login window
    if let Some(window) = app_handle.get_webview_window("market_login_window") {
        let _ = window.close();
    }

    // 4. Emit a success event to update the main app's React state
    let _ = app_handle.emit("auth_success", slug.clone());

    println!("[WFU-Auth] JWT captured and validated. Account: {}", slug);
    Ok(slug)
}

#[tauri::command]
pub async fn cancel_login(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    if let Some(window) = app_handle.get_webview_window("market_login_window") {
        let _ = window.close();
        println!("[WFU-Auth] Login window closed by user cancellation.");
    }
    Ok(())
}

#[tauri::command]
pub async fn logout(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    // 1. Clear saved token from settings file
    save_jwt(SETTINGS_PATH, "").await?;
    println!("[WFU-Auth] Token cleared from disk settings.");

    // 2. Clear all browsing data (cookies, storage, etc.) from the WebView profile
    //    so the next spawned login window is 100% clean and won't auto-authenticate.
    if let Some(window) = app_handle.get_webview_window("main") {
        match window.clear_all_browsing_data() {
            Ok(_) => println!(
                "[WFU-Auth] Successfully cleared all WebView browsing data, cookies, and local storage! Next login will be clean."
            ),
            Err(e) => {
                eprintln!(
                    "[WFU-Auth] Failed to clear WebView browsing data: {}. Falling back to manual cookie deletion...",
                    e
                );

                // Fallback: Delete all cookies for warframe.market manually
                let target_url_str = "https://warframe.market";
                let parsed_url = match tauri::Url::parse(target_url_str) {
                    Ok(u) => u,
                    Err(_) => return Ok(()),
                };

                if let Ok(cookies) = window.cookies_for_url(parsed_url) {
                    for cookie in cookies {
                        let _ = window.delete_cookie(cookie);
                    }
                    println!("[WFU-Auth] Fallback cookie deletion completed.");
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn update_standing(
    faction_key: String,
    standing: i32,
    rank: Option<i32>,
) -> Result<SyndicateState, AppError> {
    let mut standings = load_all_standings(STATUS_PATH).await?;

    let updated_state = if let Some(state) = standings.get_mut(&faction_key) {
        if let Some(r) = rank {
            state.rank = r;
        }
        let max_val = state.max_standing();
        state.standing = i32::min(i32::max(0, standing), max_val);
        state.clone()
    } else {
        return Err(AppError::Other(format!(
            "Faction '{}' not found",
            faction_key
        )));
    };

    save_all_standings(STATUS_PATH, &standings).await?;
    Ok(updated_state)
}

#[tauri::command]
pub async fn publish_syndicate(faction_key: String) -> Result<Vec<String>, AppError> {
    let standings = load_all_standings(STATUS_PATH).await?;
    let state = standings
        .get(&faction_key)
        .ok_or_else(|| AppError::Other(format!("Faction '{}' not found", faction_key)))?;

    let token = load_jwt(SETTINGS_PATH).await?.ok_or_else(|| {
        AppError::Other(
            "Not authenticated with warframe.market. Please set token first.".to_string(),
        )
    })?;

    let client = MarketClient::new(&token);
    client.validate_token().await?;
    let available_mods = get_syndicate_mods(&faction_key, state.rank);

    sync_listings(&client, state).await?;

    Ok(available_mods.into_iter().map(|s| s.to_string()).collect())
}

#[tauri::command]
pub async fn record_sale(
    item_slug: String,
    quantity: i32,
    faction_choice: Option<String>,
) -> Result<String, AppError> {
    let mut standings = load_all_standings(STATUS_PATH).await?;
    let token = load_jwt(SETTINGS_PATH).await?.ok_or_else(|| {
        AppError::Other(
            "Not authenticated with warframe.market. Please set token first.".to_string(),
        )
    })?;

    let client = MarketClient::new(&token);
    client.validate_token().await?;
    let mut eligible_factions = Vec::new();

    for (key, state) in &standings {
        let available_mods = get_syndicate_mods(key, state.rank);
        let cost = get_syndicate_cost(key);

        if available_mods.contains(&item_slug.as_str()) && state.standing >= (cost * quantity) {
            eligible_factions.push(key.clone());
        }
    }

    if eligible_factions.is_empty() {
        return Err(AppError::InsufficientStanding { item_slug });
    }

    let chosen_faction = if eligible_factions.len() > 1 {
        match faction_choice {
            Some(ref choice) => {
                if eligible_factions.contains(choice) {
                    choice.clone()
                } else {
                    return Err(AppError::Other(format!(
                        "Chosen faction '{}' is not eligible to attribute this sale.",
                        choice
                    )));
                }
            }
            None => {
                return Err(AppError::Overlap {
                    item_slug,
                    eligible_factions,
                });
            }
        }
    } else {
        eligible_factions[0].clone()
    };

    // Process Deduction
    let cost = get_syndicate_cost(&chosen_faction);
    if let Some(state) = standings.get_mut(&chosen_faction) {
        state.standing -= cost * quantity;
    }

    save_all_standings(STATUS_PATH, &standings).await?;

    // Cascade Updates — sync listings atomically; if this fails the sale is
    // still committed to disk but the caller receives the error.
    let updated_state = standings.get(&chosen_faction).unwrap();
    sync_listings(&client, updated_state).await?;

    Ok(chosen_faction)
}

#[tauri::command]
pub fn get_faction_mods(faction_key: String, rank: i32) -> Result<Vec<String>, AppError> {
    let mods = get_syndicate_mods(&faction_key, rank);
    Ok(mods.into_iter().map(|s| s.to_string()).collect())
}

/// Returns the sorted union of every mod slug available across all syndicates
/// at their maximum rank. Used by the frontend sale dropdown — replaces the
/// previously hardcoded ALL_MODS constant in App.tsx.
#[tauri::command]
pub fn get_all_mods() -> Result<Vec<String>, AppError> {
    let factions = [
        "steel_meridian",
        "arbiters_of_hexis",
        "cephalon_suda",
        "perrin_sequence",
        "red_veil",
        "new_loka",
    ];
    let mut all_mods = std::collections::HashSet::new();
    for faction in &factions {
        for m in get_syndicate_mods(faction, 5) {
            all_mods.insert(m.to_string());
        }
    }
    let mut mods: Vec<String> = all_mods.into_iter().collect();
    mods.sort();
    Ok(mods)
}
