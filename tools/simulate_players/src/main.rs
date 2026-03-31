mod api;
mod bot;
mod config;
mod cost;
mod game_state;
mod websocket;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;

/// Simulate bot-players for the Yak Mania game.
///
/// Each bot is fully independent: its own HTTP client, WebSocket subscription,
/// and decision-making. Designed to replicate 240 humans playing simultaneously
/// on smartphones.
#[derive(Parser, Debug)]
#[command(name = "simulate_players", version)]
struct Cli {
    /// AppSync GraphQL HTTPS endpoint URL
    #[arg(long)]
    api_endpoint: String,

    /// AppSync API key (x-api-key header)
    #[arg(long)]
    api_key: String,

    /// Number of bot-players to simulate
    #[arg(short = 'p', long, default_value_t = 20)]
    players: usize,

    /// Path to persistent player roster file
    #[arg(short = 'c', long, default_value = "./simulate_players.config.txt")]
    config: String,

    /// Register players and exit without playing
    #[arg(long, default_value_t = false)]
    register_only: bool,
}

#[tokio::main]
async fn main() {
    // Initialize logging with microsecond timestamps
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,tracing::span=warn"),
    )
    .format_timestamp_micros()
    .init();

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let cli = Cli::parse();

    log::info!("Yak Mania Player Simulator -- {} player(s)", cli.players);

    // Shared cost metrics (tracks all AWS operations across the entire run)
    let metrics = Arc::new(cost::CostMetrics::new());
    let players = {
        // Create the shared API caller for registration
        let api = api::ApiCaller::new(
            cli.api_endpoint.clone(),
            cli.api_key.clone(),
            metrics.clone(),
        );

        // Load or register players
        config::get_players(&cli.config, cli.players, &api).await
    };

    if players.is_empty() {
        log::error!("No players available. Exiting.");
        std::process::exit(1);
    }
    log::info!("Ready with {} player(s)", players.len());

    if cli.register_only {
        log::info!("--register-only mode: exiting after registration");
        return;
    }

    // Set up Ctrl+C handling
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    // First Ctrl+C: graceful shutdown. Second: force exit.
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
        log::info!("Ctrl+C received, initiating graceful shutdown...");
        shutdown_clone.store(true, Ordering::Relaxed);

        tokio::signal::ctrl_c()
            .await
            .expect("failed to install second Ctrl+C handler");
        log::warn!("Second Ctrl+C received, forcing exit!");
        std::process::exit(1);
    });

    // Spawn one task per player
    let graphql_url: Arc<str> = Arc::from(cli.api_endpoint.as_str());
    let api_key: Arc<str> = Arc::from(cli.api_key.as_str());

    let mut handles = Vec::with_capacity(players.len());
    for player in players {
        let url = graphql_url.clone();
        let key = api_key.clone();
        let shutdown = shutdown.clone();
        let metrics = metrics.clone();
        handles.push(tokio::spawn(bot::run(player, url, key, shutdown, metrics)));
    }

    // Await all tasks
    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => log::error!("Bot task panicked: {e}"),
        }
    }

    // Print final summary
    print_summary(results);
    cost::print_cost_report(Arc::into_inner(metrics).expect("at this point it is the only one"));
}

fn print_summary(mut results: Vec<bot::BotResult>) {
    println!();
    println!("=== Simulation Results ===");
    println!(
        " {:>3}  {:<12} {:>10}  {:>5}  {:>5}  {:>7}  {:>9}",
        "#", "Name", "Balance", "Bred", "Drove", "Sheared", "Preferred"
    );
    // Sort by balance, descending
    results.sort_by(|a, b| b.balance.total_cmp(&a.balance));
    for (i, r) in results.iter().enumerate() {
        println!(
            " {:>3}  {:<12} {:>10.2}  {:>5}  {:>5}  {:>7}  {:>9}",
            i + 1,
            r.name,
            r.balance,
            r.jobs_completed[0],
            r.jobs_completed[1],
            r.jobs_completed[2],
            r.preferred_job,
        );
    }
}
