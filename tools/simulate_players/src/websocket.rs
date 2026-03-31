use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_tungstenite::tungstenite::{self, ClientRequestBuilder, Message, http::Uri};

use crate::cost::CostMetrics;
use crate::game_state::GameUpdate;

type SubscriptionDataHandler = fn(Value) -> Result<GameUpdate, serde_json::Error>;
type SubscriptionsMap = HashMap<String, SubscriptionDataHandler>;
const SUBSCRIPTIONS: &[(&str, SubscriptionDataHandler, &str)] = &[GAME_UPDATED_SUBSCRIPTION];
const GAME_UPDATED_SUBSCRIPTION: (&str, SubscriptionDataHandler, &str) = (
    "gameUpdated",
    |mut data| {
        serde_json::from_value::<GameUpdate>(data.get_mut("gameUpdated").map(Value::take).unwrap())
    },
    r#"
    subscription {
        gameUpdated {
            sampled
            game_status
            player {
                id
                name
                balance
                yak_bred
                yak_driven
                yak_sheared
            }
            yak_counts {
                in_nursery
                with_breeders
                in_warehouse
                with_drivers
                in_shearingshed
                with_shearers
                total_sheared
            }
            job_fees {
                breeder
                driver
                shearer
            }
        }
    }
"#,
);

/// Errors from the WebSocket layer.
#[derive(Debug)]
#[allow(dead_code)]
pub enum WebSocketError {
    Connection(String),
    Protocol(String),
}

impl std::fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSocketError::Connection(e) => write!(f, "WebSocket connection error: {e}"),
            WebSocketError::Protocol(e) => write!(f, "WebSocket protocol error: {e}"),
        }
    }
}

impl std::error::Error for WebSocketError {}

/// Handle for clean shutdown of the WebSocket connection.
pub struct WebSocketHandle {
    shutdown_tx: mpsc::Sender<()>,
}

impl WebSocketHandle {
    /// Signal the WebSocket reader task to unsubscribe and close.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(()).await;
    }
}

/// Derive the AppSync real-time WebSocket URL from the GraphQL HTTP endpoint.
fn derive_ws_url(graphql_url: &str, api_key: &str) -> Result<Uri, WebSocketError> {
    let parsed = url::Url::parse(graphql_url)
        .map_err(|e| WebSocketError::Connection(format!("invalid URL: {e}")))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| WebSocketError::Connection("no host in URL".to_string()))?;

    // Replace appsync-api with appsync-realtime-api
    let ws_host = host.replace("appsync-api", "appsync-realtime-api");

    // Build the header JSON for the query parameter
    let header_json = serde_json::json!({
        "host": host,
        "x-api-key": api_key,
    });

    let header_b64 = BASE64.encode(header_json.to_string().as_bytes());

    Ok(Uri::builder()
        .scheme("wss")
        .authority(ws_host)
        .path_and_query(format!("/graphql?header={header_b64}&payload=e30="))
        .build()
        .map_err(|e| WebSocketError::Connection(format!("uri build error: {e}")))?)
}

/// Build the subscription start message.
fn build_subscribe_message(
    subscription_id: &str,
    graphql_host: &str,
    api_key: &str,
    query: &str,
) -> String {
    let msg = serde_json::json!({
        "id": subscription_id,
        "type": "start",
        "payload": {
            "data": serde_json::json!({
                "query": query,
                "variables": {}
            }).to_string(),
            "extensions": {
                "authorization": {
                    "host": graphql_host,
                    "x-api-key": api_key,
                }
            }
        }
    });
    msg.to_string()
}

/// Connect to AppSync WebSocket and subscribe to gameUpdated.
/// Returns a receiver that yields GameUpdate messages and a handle for shutdown.
pub async fn connect_and_subscribe(
    graphql_url: &str,
    api_key: &str,
    bot_name: &str,
    metrics: Arc<CostMetrics>,
) -> Result<
    (
        mpsc::UnboundedReceiver<GameUpdate>,
        WebSocketHandle,
        JoinHandle<()>,
    ),
    WebSocketError,
> {
    let (update_tx, update_rx) = mpsc::unbounded_channel();
    let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

    let graphql_url = graphql_url.to_string();
    let api_key = api_key.to_string();
    let bot_name = bot_name.to_string();

    // Spawn the WebSocket reader task
    let join_handle = tokio::spawn(async move {
        ws_reader_loop(
            graphql_url,
            api_key,
            bot_name,
            update_tx,
            shutdown_rx,
            metrics,
        )
        .await;
    });

    let handle = WebSocketHandle { shutdown_tx };
    Ok((update_rx, handle, join_handle))
}

/// The WebSocket reader loop with reconnection logic.
async fn ws_reader_loop(
    graphql_url: String,
    api_key: String,
    bot_name: String,
    update_tx: mpsc::UnboundedSender<GameUpdate>,
    mut shutdown_rx: mpsc::Receiver<()>,
    metrics: Arc<CostMetrics>,
) {
    let mut backoff_ms: u64 = 1000;

    loop {
        match ws_connect_and_run(
            &graphql_url,
            &api_key,
            &bot_name,
            &update_tx,
            &mut shutdown_rx,
            &metrics,
        )
        .await
        {
            WsLoopResult::Shutdown => {
                log::debug!("{bot_name} WebSocket reader shutting down");
                return;
            }
            WsLoopResult::Disconnected(reason) => {
                log::warn!("{bot_name} WebSocket disconnected: {reason}");
                // Exponential backoff with jitter
                let jitter_factor = 0.75 + rand::random::<f64>() * 0.5; // +/-25%
                let wait = Duration::from_millis((backoff_ms as f64 * jitter_factor) as u64);
                log::info!("{bot_name} reconnecting in {:.1}s...", wait.as_secs_f64());

                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = shutdown_rx.recv() => {
                        log::debug!("{bot_name} WebSocket reader shutting down during backoff");
                        return;
                    }
                }

                backoff_ms = (backoff_ms * 2).min(30_000);
            }
        }
    }
}

enum WsLoopResult {
    Shutdown,
    Disconnected(String),
}

/// Single WebSocket connection attempt: connect, subscribe, read messages.
async fn ws_connect_and_run(
    graphql_url: &str,
    api_key: &str,
    bot_name: &str,
    update_tx: &mpsc::UnboundedSender<GameUpdate>,
    shutdown_rx: &mut mpsc::Receiver<()>,
    metrics: &CostMetrics,
) -> WsLoopResult {
    // Derive WebSocket URL
    let ws_uri = match derive_ws_url(graphql_url, api_key) {
        Ok(url) => url,
        Err(e) => return WsLoopResult::Disconnected(e.to_string()),
    };

    log::debug!("{bot_name} connecting to WebSocket...");

    // Connect
    let (ws_stream, _response) = match tokio_tungstenite::connect_async(
        ClientRequestBuilder::new(ws_uri).with_header("Sec-WebSocket-Protocol", "graphql-ws"),
    )
    .await
    {
        Ok(s) => s,
        Err(e) => return WsLoopResult::Disconnected(format!("connect failed: {e}")),
    };

    log::debug!("{bot_name} WebSocket connected");

    let (mut write, mut read) = ws_stream.split();

    // Send connection_init
    let init_msg = serde_json::json!({"type": "connection_init"}).to_string();
    if let Err(e) = write.send(Message::Text(init_msg.into())).await {
        return WsLoopResult::Disconnected(format!("send connection_init: {e}"));
    }

    // Wait for connection_ack
    let connection_timeout_ms = match wait_for_connection_ack(&mut read, bot_name).await {
        Ok(ms) => ms,
        Err(e) => return WsLoopResult::Disconnected(e),
    };

    log::debug!("{bot_name} connection_ack received (timeout={connection_timeout_ms}ms)");

    // Track connection start time for cost metering
    let connection_start = std::time::Instant::now();

    // Subscribe
    let mut subscriptions: SubscriptionsMap = HashMap::new();

    let parsed = url::Url::parse(graphql_url).unwrap();
    let graphql_host = parsed.host_str().unwrap().to_string();
    for (sub_name, handler, query) in SUBSCRIPTIONS {
        let subscription_id = uuid::Uuid::new_v4().to_string();
        let subscribe_msg =
            build_subscribe_message(&subscription_id, &graphql_host, api_key, query);
        if let Err(e) = write.send(Message::Text(subscribe_msg.into())).await {
            return WsLoopResult::Disconnected(format!("send subscribe: {e}"));
        }
        // Wait for start_ack
        match wait_for_start_ack(&mut read, &subscription_id, bot_name).await {
            Ok(()) => {
                metrics.ws_messages_received.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => return WsLoopResult::Disconnected(e),
        }

        subscriptions.insert(subscription_id, *handler);

        log::info!("{bot_name} subscribed to {sub_name}");
    }

    // Message loop with keep-alive monitoring
    let ka_timeout = Duration::from_millis(connection_timeout_ms);

    let result = loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                for (subscription_id, _) in subscriptions.drain() {
                    let stop_msg = serde_json::json!({
                        "type": "stop",
                        "id": subscription_id,
                    })
                    .to_string();
                    let _ = write.send(Message::Text(stop_msg.into())).await;
                    metrics.ws_messages_received.fetch_add(1, Ordering::Relaxed);
                    // Record connection duration for cost metering
                    let elapsed_ms = connection_start.elapsed().as_millis() as u64;
                    metrics
                        .ws_connection_ms
                        .fetch_add(elapsed_ms, Ordering::Relaxed);
                }
                let _ = write.send(Message::Close(None)).await;
                break WsLoopResult::Shutdown;
            }
            msg = tokio::time::timeout(ka_timeout, read.next()) => {
                match msg {
                    Err(_) => {
                        // Keep-alive timeout
                        break WsLoopResult::Disconnected("keep-alive timeout".to_string());
                    }
                    Ok(None) => {
                        break WsLoopResult::Disconnected("stream ended".to_string());
                    }
                    Ok(Some(Err(e))) => {
                        break WsLoopResult::Disconnected(format!("read error: {e}"));
                    }
                    Ok(Some(Ok(msg))) => {
                        match msg {
                            Message::Text(text) => {
                                handle_ws_message(
                                    &text,
                                    bot_name,
                                    update_tx,
                                    &subscriptions,
                                    metrics,
                                );
                            }
                            Message::Close(_) => {
                                break WsLoopResult::Disconnected("server closed connection".to_string());
                            }
                            _ => {
                                // Ping/Pong/Binary - ignore
                            }
                        }
                    }
                }
            }
        }
    };

    result
}

/// Wait for the connection_ack message and return the connectionTimeoutMs value.
async fn wait_for_connection_ack<S>(read: &mut S, bot_name: &str) -> Result<u64, String>
where
    S: StreamExt<Item = Result<Message, tungstenite::Error>> + Unpin,
{
    let timeout = Duration::from_secs(30);
    loop {
        match tokio::time::timeout(timeout, read.next()).await {
            Err(_) => return Err("timeout waiting for connection_ack".to_string()),
            Ok(None) => return Err("stream ended before connection_ack".to_string()),
            Ok(Some(Err(e))) => return Err(format!("read error: {e}")),
            Ok(Some(Ok(msg))) => {
                if let Message::Text(text) = msg {
                    log::trace!("{bot_name} recv: {text}");
                    let json: Value =
                        serde_json::from_str(&text).map_err(|e| format!("parse ack: {e}"))?;
                    if json.get("type").and_then(|v| v.as_str()) == Some("connection_ack") {
                        let timeout_ms = json
                            .get("payload")
                            .and_then(|p| p.get("connectionTimeoutMs"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(300_000);
                        return Ok(timeout_ms);
                    }
                }
            }
        }
    }
}

/// Wait for the start_ack message for our subscription.
async fn wait_for_start_ack<S>(
    read: &mut S,
    subscription_id: &str,
    bot_name: &str,
) -> Result<(), String>
where
    S: StreamExt<Item = Result<Message, tungstenite::Error>> + Unpin,
{
    let timeout = Duration::from_secs(30);
    loop {
        match tokio::time::timeout(timeout, read.next()).await {
            Err(_) => return Err("timeout waiting for start_ack".to_string()),
            Ok(None) => return Err("stream ended before start_ack".to_string()),
            Ok(Some(Err(e))) => return Err(format!("read error: {e}")),
            Ok(Some(Ok(msg))) => {
                if let Message::Text(text) = msg {
                    log::trace!("{bot_name} recv: {text}");
                    let json: Value =
                        serde_json::from_str(&text).map_err(|e| format!("parse start_ack: {e}"))?;
                    let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let msg_id = json.get("id").and_then(|v| v.as_str()).unwrap_or("");

                    if msg_type == "start_ack" && msg_id == subscription_id {
                        return Ok(());
                    }
                    if msg_type == "error" {
                        return Err(format!(
                            "subscription error: {}",
                            json.get("payload")
                                .map(|v| v.to_string())
                                .unwrap_or_default()
                        ));
                    }
                    // Keep-alive or other messages during handshake - continue
                }
            }
        }
    }
}

/// Handle a single WebSocket text message.
fn handle_ws_message(
    text: &str,
    bot_name: &str,
    update_tx: &mpsc::UnboundedSender<GameUpdate>,
    subscriptions: &SubscriptionsMap,
    metrics: &CostMetrics,
) {
    log::trace!("{bot_name} ws recv: {text}");

    let mut json: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("{bot_name} failed to parse WebSocket message: {e}");
            return;
        }
    };

    let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match msg_type {
        "ka" => {
            // Keep-alive: the timeout in the select loop is automatically reset
            log::trace!("{bot_name} keep-alive received");
        }
        "data" => {
            metrics.ws_messages_received.fetch_add(1, Ordering::Relaxed);

            // Extract gameUpdated from payload.data.gameUpdated
            if let Some(data) = json
                .get_mut("payload")
                .and_then(|p| p.get_mut("data").map(Value::take))
                && let Some(handler) = subscriptions.get(json.get("id").unwrap().as_str().unwrap())
            {
                match handler(data) {
                    Ok(update) => {
                        if let Err(_) = update_tx.send(update) {
                            log::debug!("{bot_name} update channel closed, dropping message");
                        }
                    }
                    Err(e) => {
                        log::warn!("{bot_name} failed to deserialize GameUpdate: {e}");
                    }
                }
            }
        }
        "error" => {
            let payload = json
                .get("payload")
                .map(|v| v.to_string())
                .unwrap_or_default();
            log::error!("{bot_name} WebSocket error: {payload}");
        }
        "complete" => {
            log::info!("{bot_name} subscription completed by server");
        }
        other => {
            log::debug!("{bot_name} unknown WebSocket message type: {other}");
        }
    }
}
