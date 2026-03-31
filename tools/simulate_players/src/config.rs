use std::path::Path;

use lambda_appsync::ID;

use crate::api::ApiCaller;
use crate::game_state::Job;

/// Persisted player configuration (one line in the config file).
#[derive(Debug, Clone)]
pub struct PlayerConfig {
    pub idx: usize,
    pub name: String,
    pub id: ID,
    pub secret: String,
    pub preferred_job: Job,
}

impl PlayerConfig {
    /// Serialize to the config file line format.
    fn to_line(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.idx, self.name, self.id, self.secret, self.preferred_job
        )
    }

    /// Parse from a config file line.
    fn from_line(line: &str) -> Result<Self, String> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(format!("expected 5 fields, got {}: {line}", parts.len()));
        }
        Ok(PlayerConfig {
            idx: parts[0].parse().map_err(|e| format!("invalid idx: {e}"))?,
            name: parts[1].to_string(),
            id: parts[2].parse().map_err(|e| format!("invalid ID: {e}"))?,
            secret: parts[3].to_string(),
            preferred_job: parts[4].parse().map_err(|e| format!("invalid job: {e}"))?,
        })
    }
}

/// Load existing players from the config file, register missing ones, write back.
pub async fn get_players(
    config_path: &str,
    requested_count: usize,
    api: &ApiCaller,
) -> Vec<PlayerConfig> {
    // Step 1: Read existing config
    let mut players = load_config(config_path);
    let existing_count = players.len();

    log::info!("Loaded {existing_count} existing player(s) from {config_path}");

    // Step 2: Take the first min(existing, requested) players
    players.truncate(requested_count);

    // Step 3: Register missing players concurrently
    if existing_count < requested_count {
        let new_start = existing_count + 1;
        let new_end = requested_count;
        log::info!(
            "Registering {} new player(s) (#{new_start} to #{new_end})...",
            new_end - new_start + 1
        );

        let mut handles = Vec::new();
        for i in new_start..=new_end {
            let api = api.clone();
            handles.push(tokio::spawn(
                async move { register_one_player(i, &api).await },
            ));
        }

        let mut new_players = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(player)) => {
                    log::info!("Registered {}", player.name);
                    new_players.push(player);
                }
                Ok(Err(e)) => {
                    log::error!("Registration failed: {e}");
                }
                Err(e) => {
                    log::error!("Registration task panicked: {e}");
                }
            }
        }
        players.extend(new_players);
    }

    // Step 4: Sort by idx
    players.sort_by_key(|p| p.idx);

    // Step 5: Write back
    if !players.is_empty() {
        save_config(config_path, &players);
    }

    players
}

/// Register a single new player via the API.
async fn register_one_player(idx: usize, api: &ApiCaller) -> Result<PlayerConfig, String> {
    use rand::seq::IndexedRandom;

    let name = format!("Bot{idx}");
    let secret = uuid::Uuid::new_v4().to_string();
    let preferred_job = *Job::all()
        .choose(&mut rand::rng())
        .expect("ALL is non-empty");

    let player_id = api
        .register_new_player(&name, &secret)
        .await
        .map_err(|e| format!("register {name}: {e}"))?;

    Ok(PlayerConfig {
        idx,
        name,
        id: player_id,
        secret,
        preferred_job,
    })
}

/// Load players from the config file. Returns empty vec if file doesn't exist.
fn load_config(path: &str) -> Vec<PlayerConfig> {
    let path = Path::new(path);
    if !path.exists() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Could not read config file: {e}");
            return Vec::new();
        }
    };
    let mut players = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match PlayerConfig::from_line(line) {
            Ok(p) => players.push(p),
            Err(e) => log::warn!("Config line {}: {e}", line_num + 1),
        }
    }
    players
}

/// Write all players back to the config file.
fn save_config(path: &str, players: &[PlayerConfig]) {
    let content: String = players
        .iter()
        .map(|p| p.to_line())
        .collect::<Vec<_>>()
        .join("\n");
    if let Err(e) = std::fs::write(path, content + "\n") {
        log::error!("Failed to write config file {path}: {e}");
    } else {
        log::info!("Saved {} player(s) to {path}", players.len());
    }
}
