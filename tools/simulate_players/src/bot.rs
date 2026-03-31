use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use lambda_appsync::ID;
use rand::Rng;
use rand::distr::Distribution;
use rand::distr::weighted::WeightedIndex;

use crate::api::{ApiCaller, ApiError};
use crate::config::PlayerConfig;
use crate::cost::CostMetrics;
use crate::game_state::*;
use crate::websocket;

/// Final statistics for a bot, returned after the task completes.
pub struct BotResult {
    pub name: String,
    pub balance: f64,
    pub jobs_completed: [u32; 3], // indexed: 0=Breeder, 1=Driver, 2=Shearer
    pub preferred_job: Job,
}
impl From<BotState> for BotResult {
    fn from(state: BotState) -> Self {
        Self {
            name: state.name,
            balance: state.balance,
            jobs_completed: state.jobs_completed,
            preferred_job: state.preferred_job,
        }
    }
}

/// Per-bot runtime state.
#[derive(Debug)]
struct BotState {
    // Identity
    player_id: ID,
    secret: String,
    name: String,
    preferred_job: Job,

    // Game knowledge
    job_fees: JobFees,
    yak_counts: YakCounts,
    game_status: GameStatus,

    // Current work
    current_work: Option<(Job, ID)>,

    // Statistics
    balance: f64,
    jobs_completed: [u32; 3],
}

impl BotState {
    fn new(config: &PlayerConfig) -> Self {
        Self {
            player_id: config.id.clone(),
            secret: config.secret.clone(),
            name: config.name.clone(),
            preferred_job: config.preferred_job,
            job_fees: JobFees {
                breeder: 0.0,
                driver: 0.0,
                shearer: 0.0,
            },
            yak_counts: YakCounts {
                in_nursery: 0,
                with_breeders: 0,
                in_warehouse: 0,
                with_drivers: 0,
                in_shearingshed: 0,
                with_shearers: 0,
                total_sheared: 0,
            },
            game_status: GameStatus::Reset,
            current_work: None,
            balance: 0.0,
            jobs_completed: [0; 3],
        }
    }

    /// Apply a GameUpdate to the local state.
    fn apply_update(&mut self, update: &GameUpdate) {
        if let Some(ref counts) = update.yak_counts {
            self.yak_counts = *counts;
        }

        if let Some(ref fees) = update.job_fees {
            self.job_fees = *fees;
        }

        if let Some(status) = update.game_status {
            self.game_status = status;
            if matches!(status, GameStatus::Reset) {
                self.balance = 0.0;
                self.current_work = None;
                return;
            }
        }

        // Update balance from player info if it's about us
        if let Some(ref player) = update.player
            && player.id == self.player_id
        {
            self.balance = player.balance;
        }
    }

    /// Apply a mutation response to local state (always about us).
    fn apply_mutation_response(&mut self, update: &GameUpdate) {
        if let Some(ref counts) = update.yak_counts {
            self.yak_counts = *counts;
        }
        if let Some(ref fees) = update.job_fees {
            self.job_fees = *fees;
        }
        if let Some(ref player) = update.player {
            self.balance = player.balance;
        }
        if let Some(game_status) = update.game_status {
            self.game_status = game_status;
        }
    }
}

/// Run a single bot. This is the top-level entry point spawned per bot-player.
pub async fn run(
    config: PlayerConfig,
    graphql_url: Arc<str>,
    api_key: Arc<str>,
    shutdown: Arc<AtomicBool>,
    metrics: Arc<CostMetrics>,
) -> BotResult {
    let name = config.name.clone();
    let api = ApiCaller::new(
        graphql_url.to_string(),
        api_key.to_string(),
        metrics.clone(),
    );
    let mut state = BotState::new(&config);

    // Connect WebSocket
    let (mut update_rx, ws_handle, ws_join_handle) = loop {
        match websocket::connect_and_subscribe(&graphql_url, &api_key, &name, metrics.clone()).await
        {
            Ok(result) => break result,
            Err(e) => {
                log::error!("{name} initial WebSocket connection failed: {e}");
                if shutdown.load(Ordering::Relaxed) {
                    return state.into();
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    log::info!("{name} connected and subscribed");

    // Main lifecycle loop
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        // Drain pending updates before making a decision
        while let Ok(update) = update_rx.try_recv() {
            state.apply_update(&update);
        }

        // If we already have a job for some reason
        if matches!(state.game_status, GameStatus::Started | GameStatus::Stopped)
            && state.current_work.is_some()
        {
            // Finish current job
            finish_current_job(&mut state, &api, &name).await;
            continue;
        }

        match state.game_status {
            GameStatus::Reset => {
                // IDLE/WAITING: wait for game start or shutdown
                log::info!("{name} waiting for game to start...");
                // Wait for game start
                loop {
                    tokio::select! {
                        msg = update_rx.recv() => {
                            match msg {
                                Some(update) => {
                                    state.apply_update(&update);
                                    if state.game_status == GameStatus::Started {
                                        log::info!("{name} game started, beginning play");
                                        break;
                                    }
                                }
                                None => {
                                    log::warn!("{name} update channel closed");
                                    break
                                }
                            }
                        }
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            if shutdown.load(Ordering::Relaxed) {
                                break
                            }
                        }
                    }
                }
            }
            GameStatus::Stopped | GameStatus::Started => {
                // PLAYING: choose job, buy, work, sell

                // Select a job
                let job = match select_job(&state).await {
                    Some(j) => j,
                    None => {
                        log::debug!("{name} no job available");
                        // No jobs available, wait for an update
                        match tokio::time::timeout(Duration::from_secs(1), update_rx.recv()).await {
                            Ok(Some(update)) => state.apply_update(&update),
                            Ok(None) => {
                                log::warn!("{name} update channel closed");
                                break;
                            }
                            Err(_) => {} // timeout, retry
                        }
                        continue;
                    }
                };

                log::info!("{name} trying to work as a {job}");
                // Execute buy-work-sell cycle
                execute_job_cycle(&mut state, &api, &name, job).await
            }
        }
    }

    ws_handle.shutdown().await;
    ws_join_handle.await.unwrap();
    log::info!("{name} exiting (balance={:.2})", state.balance);
    state.into()
}

// ── Job selection ─────────────────────────────────────────────────────

async fn select_job(state: &BotState) -> Option<Job> {
    let available: Vec<Job> = Job::all()
        .into_iter()
        .filter(|job| match job {
            Job::Breeder => !state.yak_counts.max_reached(), // Breeders can't buy during drain
            Job::Driver => state.yak_counts.in_warehouse > 0,
            Job::Shearer => state.yak_counts.in_shearingshed > 0,
        })
        .collect();

    if available.is_empty() {
        return None;
    }

    // Hesitate (sleep)
    // Bot1 is quicker
    let duration = if state.name == "Bot1" {
        rand::rng().random_range(0.5..=1.25)
    } else {
        rand::rng().random_range(0.75..=1.5)
    };
    tokio::time::sleep(Duration::from_secs_f64(duration)).await;

    let weights: Vec<f64> = available
        .iter()
        .map(|&job| {
            let fee = state.job_fees[job];
            let base_weight = fee.max(0.0);
            let preferred_multiplier = if job == state.preferred_job { 1.1 } else { 1.0 };
            base_weight * preferred_multiplier
        })
        .collect();

    let dist = WeightedIndex::new(&weights).ok()?;
    let mut rng = rand::rng();
    let chosen = available[dist.sample(&mut rng)];

    log::debug!(
        "{} selected job {} (fee={:.1}, weight={:.1})",
        state.name,
        chosen,
        state.job_fees[chosen],
        weights[available.iter().position(|&j| j == chosen).unwrap()]
    );

    Some(chosen)
}

// ── Job cycle execution ───────────────────────────────────────────────

async fn execute_job_cycle(state: &mut BotState, api: &ApiCaller, name: &str, job: Job) {
    // Buy
    let buy_result = retry_mutation(
        || api.buy_for_job(job, state.player_id, &state.secret),
        name,
        "buy",
    )
    .await;

    match buy_result {
        Ok(update) => {
            let yak_id = update
                .player
                .as_ref()
                .and_then(|p| p.assignment.as_ref())
                .map(|a| a.yak.id.clone())
                .unwrap_or_default();
            log::debug!("{name} buy {job} -> yak_id={yak_id}");
            state.apply_mutation_response(&update);
            state.current_work = Some((job, yak_id));
        }
        Err(e) => {
            log::error!("{name} buy {job} failed: {e}");
            return; // Skip this cycle
        }
    }

    // Work (sleep)
    let is_preferred = job == state.preferred_job;
    // Bot1 is quicker
    let (min_s, max_s) = if is_preferred { (5.0, 7.0) } else { (6.0, 8.0) };
    let (min_s, max_s) = if state.name == "Bot1" {
        (min_s - 1.0, max_s - 1.0)
    } else {
        (min_s, max_s)
    };
    let duration = rand::rng().random_range(min_s..=max_s);
    log::debug!("{name} working for {duration:.2}s (preferred={is_preferred})");

    // Sleep
    tokio::time::sleep(Duration::from_secs_f64(duration)).await;

    // Sell
    let (current_job, yak_id) = state.current_work.take().unwrap();

    let sell_result = retry_mutation(
        || api.sell_for_job(current_job, state.player_id, &state.secret, yak_id),
        name,
        "sell",
    )
    .await;

    match sell_result {
        Ok(update) => {
            state.apply_mutation_response(&update);
            state.jobs_completed[current_job.index()] += 1;
            log::info!(
                "{name} sold yak as a {current_job} -> balance={:.2}",
                state.balance
            );
        }
        Err(e) => {
            log::error!("{name} sell {current_job} yak={yak_id} failed: {e}");
        }
    }
}

// ── Draining ──────────────────────────────────────────────────────────

async fn finish_current_job(state: &mut BotState, api: &ApiCaller, name: &str) {
    if let Some((job, yak_id)) = state.current_work.take() {
        log::info!("{name} draining: finishing {job} for yak {yak_id}");
        let sell_result = retry_mutation(
            || api.sell_for_job(job, state.player_id, &state.secret, yak_id),
            name,
            "drain-sell",
        )
        .await;
        match sell_result {
            Ok(update) => {
                state.apply_mutation_response(&update);
                state.jobs_completed[job.index()] += 1;
                log::info!("{name} drain sell {job} -> balance={:.2}", state.balance);
            }
            Err(e) => {
                log::error!("{name} drain sell failed: {e}");
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Retry a mutation up to 3 times on network errors.
async fn retry_mutation<F, Fut>(
    mut make_call: F,
    bot_name: &str,
    label: &str,
) -> Result<GameUpdate, ApiError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<GameUpdate, ApiError>>,
{
    let mut retries = 0;
    loop {
        match make_call().await {
            Ok(result) => return Ok(result),
            Err(ApiError::Network(e)) if retries < 3 => {
                retries += 1;
                log::error!("{bot_name} {label} network error (retry {retries}/3): {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e @ ApiError::GraphQL { .. })
                if !e.is_no_more_yak()
                    && !e.is_invalid_game_status()
                    && !e.is_invalid_player_status()
                    && retries < 1 =>
            {
                retries += 1;
                log::error!("{bot_name} {label} GraphQL error (retry {retries}/1): {e}");
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

// /// Resync bot state from the server via a gameState query.
// async fn resync_state(state: &mut BotState, api: &ApiCaller, name: &str) {
//     log::info!("{name} resyncing state from server...");
//     match api.query_game_state().await {
//         Ok(gs) => {
//             state.game_status = gs.game_status;
//             state.yak_counts = gs.yak_counts;
//             state.job_fees = gs.job_fees;
//             state.current_job = None;
//             state.current_yak_id = None;
//             log::info!("{name} resync complete: status={:?}", state.game_status);
//         }
//         Err(e) => {
//             log::error!("{name} resync failed: {e}");
//         }
//     }
// }
