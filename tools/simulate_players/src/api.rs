use std::sync::Arc;
use std::sync::atomic::Ordering;

use lambda_appsync::ID;
use serde_json::Value;

use crate::cost::CostMetrics;
use crate::game_state::{GameUpdate, Job};

/// Errors from the API layer.
#[derive(Debug)]
pub enum ApiError {
    /// Network or HTTP-level error.
    Network(String),
    /// GraphQL error returned by the server.
    GraphQL { error_type: String, message: String },
    /// Failed to parse the response.
    Parse(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "network error: {e}"),
            ApiError::GraphQL {
                error_type,
                message,
            } => write!(f, "GraphQL error [{error_type}]: {message}"),
            ApiError::Parse(e) => write!(f, "parse error: {e}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    pub fn is_no_more_yak(&self) -> bool {
        matches!(self, ApiError::GraphQL { error_type, .. } if error_type == "NoMoreYak")
    }

    pub fn is_invalid_game_status(&self) -> bool {
        matches!(self, ApiError::GraphQL { error_type, .. } if error_type == "InvalidGameStatus")
    }

    pub fn is_invalid_player_status(&self) -> bool {
        matches!(self, ApiError::GraphQL { error_type, .. } if error_type == "InvalidPlayerStatus")
    }
}

#[derive(Debug, Clone)]
pub struct ApiCaller {
    client: reqwest::Client,
    url: Arc<str>,
    key: Arc<str>,
    metrics: Arc<CostMetrics>,
}

impl ApiCaller {
    pub fn new(url: String, key: String, metrics: Arc<CostMetrics>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            url: Arc::from(url.as_str()),
            key: Arc::from(key.as_str()),
            metrics,
        }
    }
    /// Execute a raw GraphQL query/mutation and return the `data` field.
    pub async fn call_graphql(&self, query: &str) -> Result<Value, ApiError> {
        self.metrics
            .graphql_mutations
            .fetch_add(1, Ordering::Relaxed);

        let body = serde_json::json!({ "query": query });

        let response = self
            .client
            .post(self.url.as_ref())
            .header("Content-Type", "application/json")
            .header("x-api-key", self.key.as_ref())
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        log::trace!("HTTP {status} response: {text}");

        let json: Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(e.to_string()))?;

        // Check for GraphQL errors
        if let Some(errors) = json.get("errors").and_then(|e| e.as_array()) {
            if let Some(first) = errors.first() {
                let error_type = first
                    .get("errorType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();
                let message = first
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("no message")
                    .to_string();
                return Err(ApiError::GraphQL {
                    error_type,
                    message,
                });
            }
        }

        json.get("data")
            .cloned()
            .ok_or_else(|| ApiError::Parse("missing 'data' field in response".to_string()))
    }

    // ── Mutation helpers ──────────────────────────────────────────────

    pub async fn register_new_player(&self, name: &str, secret: &str) -> Result<ID, ApiError> {
        self.metrics
            .register_new_player
            .fetch_add(1, Ordering::Relaxed);

        let query = format!(
            r#"mutation {{
                registerNewPlayer(name: "{name}", secret: "{secret}") {{
                    sampled
                    player {{
                        id
        				name
        				balance
        				yak_bred
        				yak_driven
        				yak_sheared
                    }}
                }}
            }}"#
        );
        let data = self.call_graphql(&query).await?;
        let player_id = data
            .get("registerNewPlayer")
            .and_then(|v| v.get("player"))
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::Parse("missing player.id in registerNewPlayer".to_string()))?
            .parse()
            .unwrap();
        Ok(player_id)
    }

    /// Generic buy mutation. Returns the parsed GameUpdate from the mutation response.
    async fn buy_mutation(
        &self,
        mutation_name: &str,
        player_id: ID,
        secret: &str,
    ) -> Result<GameUpdate, ApiError> {
        let query = format!(
            r#"mutation {{
                {mutation_name}(player_id: "{player_id}", secret: "{secret}") {{
                    sampled
                    player {{
                        id
                        name
                        assignment {{
                            job
                            yak {{
                                id
                            }}
                            fee
                        }}
                        balance
                        yak_bred
                        yak_driven
                        yak_sheared
                    }}
                    yak_counts {{
                        in_nursery
                        with_breeders
                        in_warehouse
                        with_drivers
                        in_shearingshed
                        with_shearers
                        total_sheared
                    }}
                    job_fees {{
                        breeder
                        driver
                        shearer
                    }}
                }}
            }}"#
        );
        let data = self.call_graphql(&query).await?;
        let update_value = data
            .get(mutation_name)
            .ok_or_else(|| ApiError::Parse(format!("missing '{mutation_name}' in response")))?;
        serde_json::from_value(update_value.clone())
            .map_err(|e| ApiError::Parse(format!("deserialize {mutation_name}: {e}")))
    }

    /// Generic sell mutation. Returns the parsed GameUpdate from the mutation response.
    async fn sell_mutation(
        &self,
        mutation_name: &str,
        player_id: ID,
        secret: &str,
        yak_id: ID,
    ) -> Result<GameUpdate, ApiError> {
        let query = format!(
            r#"mutation {{
                {mutation_name}(player_id: "{player_id}", secret: "{secret}", yak_id: "{yak_id}") {{
                    sampled
                    game_status
                    player {{
                        id
                        name
                        assignment {{
                            job
                            yak {{
                                id
                            }}
                            fee
                        }}
                        balance
                        yak_bred
                        yak_driven
                        yak_sheared
                    }}
                    yak_counts {{
                        in_nursery
                        with_breeders
                        in_warehouse
                        with_drivers
                        in_shearingshed
                        with_shearers
                        total_sheared
                    }}
                    job_fees {{
                        breeder
                        driver
                        shearer
                    }}
                }}
            }}"#
        );
        let data = self.call_graphql(&query).await?;
        let update_value = data
            .get(mutation_name)
            .ok_or_else(|| ApiError::Parse(format!("missing '{mutation_name}' in response")))?;
        serde_json::from_value(update_value.clone())
            .map_err(|e| ApiError::Parse(format!("deserialize {mutation_name}: {e}")))
    }

    // ── Job-specific buy/sell ─────────────────────────────────────────

    pub async fn buy_baby_yak(&self, player_id: ID, secret: &str) -> Result<GameUpdate, ApiError> {
        self.metrics.buy_baby_yak.fetch_add(1, Ordering::Relaxed);

        self.buy_mutation("buyBabyYak", player_id, secret).await
    }

    pub async fn sell_grown_yak(
        &self,
        player_id: ID,
        secret: &str,
        yak_id: ID,
    ) -> Result<GameUpdate, ApiError> {
        self.metrics.sell_grown_yak.fetch_add(1, Ordering::Relaxed);

        self.sell_mutation("sellGrownYak", player_id, secret, yak_id)
            .await
    }

    pub async fn buy_grown_yak(&self, player_id: ID, secret: &str) -> Result<GameUpdate, ApiError> {
        self.metrics.buy_grown_yak.fetch_add(1, Ordering::Relaxed);

        self.buy_mutation("buyGrownYak", player_id, secret).await
    }

    pub async fn sell_unsheared_yak(
        &self,
        player_id: ID,
        secret: &str,
        yak_id: ID,
    ) -> Result<GameUpdate, ApiError> {
        self.metrics
            .sell_unsheared_yak
            .fetch_add(1, Ordering::Relaxed);

        self.sell_mutation("sellUnshearedYak", player_id, secret, yak_id)
            .await
    }

    pub async fn buy_unsheared_yak(
        &self,
        player_id: ID,
        secret: &str,
    ) -> Result<GameUpdate, ApiError> {
        self.metrics
            .buy_unsheared_yak
            .fetch_add(1, Ordering::Relaxed);

        self.buy_mutation("buyUnshearedYak", player_id, secret)
            .await
    }

    pub async fn sell_sheared_yak(
        &self,
        player_id: ID,
        secret: &str,
        yak_id: ID,
    ) -> Result<GameUpdate, ApiError> {
        self.metrics
            .sell_sheared_yak
            .fetch_add(1, Ordering::Relaxed);

        self.sell_mutation("sellShearedYak", player_id, secret, yak_id)
            .await
    }

    /// Convenience: buy for the given job.
    pub async fn buy_for_job(
        &self,
        job: Job,
        player_id: ID,
        secret: &str,
    ) -> Result<GameUpdate, ApiError> {
        match job {
            Job::Breeder => self.buy_baby_yak(player_id, secret).await,
            Job::Driver => self.buy_grown_yak(player_id, secret).await,
            Job::Shearer => self.buy_unsheared_yak(player_id, secret).await,
        }
    }

    /// Convenience: sell for the given job.
    pub async fn sell_for_job(
        &self,
        job: Job,
        player_id: ID,
        secret: &str,
        yak_id: ID,
    ) -> Result<GameUpdate, ApiError> {
        match job {
            Job::Breeder => self.sell_grown_yak(player_id, secret, yak_id).await,
            Job::Driver => self.sell_unsheared_yak(player_id, secret, yak_id).await,
            Job::Shearer => self.sell_sheared_yak(player_id, secret, yak_id).await,
        }
    }

    // /// Query the full game state (used for resync on error).
    // pub async fn query_game_state(&self) -> Result<GameState, ApiError> {
    //     let query = r#"query {
    //             gameState {
    //                 game_status
    //                 yak_counts {
    //                     with_breeders
    //                     in_warehouse
    //                     with_drivers
    //                     in_shearingshed
    //                     with_shearers
    //                     desired_yak_count
    //                 }
    //                 job_fees {
    //                     breeder
    //                     driver
    //                     shearer
    //                 }
    //             }
    //         }"#;
    //     let data = self.call_graphql(query).await?;
    //     let gs = data
    //         .get("gameState")
    //         .ok_or_else(|| ApiError::Parse("missing 'gameState' in response".to_string()))?;
    //     serde_json::from_value(gs.clone())
    //         .map_err(|e| ApiError::Parse(format!("deserialize gameState: {e}")))
    // }
}
