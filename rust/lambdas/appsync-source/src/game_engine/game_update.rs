use super::{GameCounts, GameStatus, GameUpdate, JobFees, Player, YakCounts};

// Builder pattern: each method takes `mut self` and returns `Self` for chaining.
// Example: GameUpdate::from(status).sampled().with_game_counts_derived(counts)
impl GameUpdate {
    pub fn sampled(mut self) -> Self {
        self.sampled = true;
        self
    }
    pub fn with_sampled(mut self, sampled: bool) -> Self {
        self.sampled = sampled;
        self
    }
    pub fn with_player(mut self, player: Player) -> Self {
        self.player = Some(player);
        self
    }
    pub fn with_yak_counts(mut self, yak_counts: YakCounts) -> Self {
        self.yak_counts = Some(yak_counts);
        self
    }
    pub fn with_game_counts_derived(self, game_counts: GameCounts) -> Self {
        let already_sampled = self.sampled;
        self.with_yak_counts(game_counts.into())
            .with_job_fees(game_counts.into())
            .with_sampled(already_sampled || game_counts.sampled())
    }
    pub fn with_game_status(mut self, game_status: GameStatus) -> Self {
        self.game_status = Some(game_status);
        self
    }
    pub fn with_job_fees(mut self, job_fees: JobFees) -> Self {
        self.job_fees = Some(job_fees);
        self
    }
}

// Multiple From implementations allow constructing GameUpdate from different source types.
// Rust's From trait enables the into() syntax used throughout the codebase.
impl From<GameStatus> for GameUpdate {
    fn from(game_status: GameStatus) -> Self {
        Self {
            game_status: Some(game_status),
            ..Default::default()
        }
    }
}
impl From<Player> for GameUpdate {
    fn from(player: Player) -> Self {
        Self {
            player: Some(player),
            ..Default::default()
        }
    }
}
impl From<YakCounts> for GameUpdate {
    fn from(yak_counts: YakCounts) -> Self {
        Self {
            yak_counts: Some(yak_counts),
            ..Default::default()
        }
    }
}
