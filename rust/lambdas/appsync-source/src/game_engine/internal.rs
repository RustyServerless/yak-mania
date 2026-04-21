use std::ops::{Deref, Index, IndexMut, Sub};

use dynamodb_facade::{
    Condition, DynamoDBItemOp, Error, KeyId, Update, UpdateSetRhs, dynamodb_item,
};
use serde::{Deserialize, Serialize};

use crate::dynamodb_table::{ItemType, MonoTable, PK, SK};

use super::{Job, JobFees, WaitingPlace, YakCounts};

// Sampler: rate-limits subscription updates to ~4/sec (250ms granularity).
// Without this, every player action would push an update to all subscribers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sampler {
    update_ms: u64,
    old_update_ms: u64,
}

impl Sampler {
    const SAMPLE_FREQUENCY_MS: u64 = 250;
    pub fn sampled(self) -> bool {
        self.update_ms / Self::SAMPLE_FREQUENCY_MS != self.old_update_ms / Self::SAMPLE_FREQUENCY_MS
    }
}

// Phantom type / typestate pattern: GameCounts<Theorical> vs GameCounts<Real>
// share the same data layout but are different types at compile time.
// This prevents accidentally mixing ideal (computed) counts with actual (from DynamoDB) counts.
pub trait GameCountType {}
#[derive(Debug, Clone, Copy, Default)]
struct Theorical;
impl GameCountType for Theorical {}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Real {
    player_count: usize,
    desired_yak_count: usize,
    sheared_yak_count: usize,
    #[serde(flatten)]
    sampler: Option<Sampler>,
}
impl GameCountType for Real {}

/// Intermediate representation maintained to compute the incentive fees for each Job.
/// The fee system compares actual yak distribution (Real) against ideal distribution (Theorical)
/// to create price incentives that guide players toward the jobs that need more workers.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct GameCounts<Type: GameCountType = Real> {
    yaks_with_player: [usize; Job::COUNT],
    yaks_waiting: [usize; WaitingPlace::COUNT],
    #[serde(flatten)]
    game_count_type: Type,
}
impl<Type: GameCountType> GameCounts<Type> {
    pub fn yaks_with_player(&self) -> usize {
        self.yaks_with_player.iter().sum()
    }
    pub fn yaks_waiting(&self) -> usize {
        self.yaks_waiting.iter().sum()
    }
    pub fn total_yaks(&self) -> usize {
        self.yaks_waiting() + self.yaks_with_player()
    }
}

impl<Type: GameCountType> Index<Job> for GameCounts<Type> {
    type Output = usize;

    fn index(&self, index: Job) -> &Self::Output {
        &self.yaks_with_player[index.index()]
    }
}
impl<Type: GameCountType> IndexMut<Job> for GameCounts<Type> {
    fn index_mut(&mut self, index: Job) -> &mut Self::Output {
        &mut self.yaks_with_player[index.index()]
    }
}

impl<Type: GameCountType> Index<WaitingPlace> for GameCounts<Type> {
    type Output = usize;

    fn index(&self, index: WaitingPlace) -> &Self::Output {
        &self.yaks_waiting[index.index()]
    }
}
impl<Type: GameCountType> IndexMut<WaitingPlace> for GameCounts<Type> {
    fn index_mut(&mut self, index: WaitingPlace) -> &mut Self::Output {
        &mut self.yaks_waiting[index.index()]
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GameCountIndex {
    Job(Job),
    WaitingPlace(WaitingPlace),
}
impl core::fmt::Display for GameCountIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameCountIndex::Job(job) => write!(f, "yaks_with_player[{}]", job.index()),
            GameCountIndex::WaitingPlace(waiting_place) => {
                write!(f, "yaks_waiting[{}]", waiting_place.index())
            }
        }
    }
}

impl<Type: GameCountType> Index<GameCountIndex> for GameCounts<Type> {
    type Output = usize;

    fn index(&self, index: GameCountIndex) -> &Self::Output {
        match index {
            GameCountIndex::Job(job) => &self.yaks_with_player[job.index()],
            GameCountIndex::WaitingPlace(waiting_place) => {
                &self.yaks_waiting[waiting_place.index()]
            }
        }
    }
}
impl<Type: GameCountType> IndexMut<GameCountIndex> for GameCounts<Type> {
    fn index_mut(&mut self, index: GameCountIndex) -> &mut Self::Output {
        match index {
            GameCountIndex::Job(job) => &mut self.yaks_with_player[job.index()],
            GameCountIndex::WaitingPlace(waiting_place) => {
                &mut self.yaks_waiting[waiting_place.index()]
            }
        }
    }
}

// Declarative macro that generates newtype array wrappers indexable by enum variants.
// Creates a struct wrapping a fixed-size array with Index, IndexMut, and Deref implementations.
macro_rules! enum_indexed {
    ($name:ident, $t:ty, $enum:ty) => {
        #[derive(Debug, Clone, Copy, Default)]
        struct $name([$t; <$enum>::COUNT]);
        impl Index<$enum> for $name {
            type Output = $t;

            fn index(&self, index: $enum) -> &Self::Output {
                &self.0[index.index()]
            }
        }
        impl IndexMut<$enum> for $name {
            fn index_mut(&mut self, index: $enum) -> &mut Self::Output {
                &mut self.0[index.index()]
            }
        }
        impl Deref for $name {
            type Target = [$t];

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}
enum_indexed!(JobDiffs, i64, Job);
#[derive(Debug)]
struct RangedJobDiffs {
    diffs: JobDiffs,
    player_count: usize,
}

impl From<RangedJobDiffs> for JobFees {
    #[tracing::instrument(name = "diffs_into_fees", ret, level = "debug")]
    fn from(value: RangedJobDiffs) -> Self {
        let mut job_fees = JobFees::neutral();
        if value.player_count > 0 {
            // For each job,
            // compute an incitation between -100 and 100 by simply scaling the diffs
            // to the min/max values (which are both the player count)
            let scale_factor = 100.0 / value.player_count as f64;
            for job in Job::all() {
                job_fees.adjust_fee(job, value.diffs[job] as f64 * scale_factor);
            }
        }
        job_fees
    }
}

enum_indexed!(JobConstraints, usize, Job);
impl JobConstraints {
    fn free_spots_left(&self) -> bool {
        for c in self.0 {
            if c > 0 {
                return true;
            }
        }
        false
    }
    fn free_spots_jobs(self) -> impl Iterator<Item = Job> {
        Job::all().into_iter().filter(move |&j| self[j] > 0)
    }
}

impl GameCounts<Theorical> {
    #[tracing::instrument(ret, level = "debug")]
    fn new_ideal(
        player_count: usize,
        desired_yak_count: usize,
        mut constaints: JobConstraints,
    ) -> Self {
        let mut game_counts = Self::default();

        // Affect all players if possible
        while let affected_players = game_counts.yaks_with_player()
            && affected_players < player_count
            && constaints.free_spots_left()
        {
            game_counts.affect_players(player_count - affected_players, &mut constaints);
        }

        let affected_players = game_counts.yaks_with_player();
        // If all players are occupied with a Yak, how many should exist in the warehouse/shearing shed
        let free_yaks = desired_yak_count.saturating_sub(affected_players);
        game_counts.affect_yaks(free_yaks);

        game_counts
    }

    fn affect_players(&mut self, player_count: usize, constaints: &mut JobConstraints) {
        // How many job remaining?
        let job_count = constaints.free_spots_jobs().count();
        // Equi-repartition
        let players_per_job = player_count / job_count;
        // Remainer (always less than free_spots_jobs count)
        let mut bonus_players = player_count % job_count;
        // For each job, add players and update constraints if needed
        for job in constaints.free_spots_jobs() {
            // We add a bonus player if the remainer is not 0
            let players_to_affect = if bonus_players > 0 {
                bonus_players -= 1;
                players_per_job + 1
            } else {
                players_per_job
            };
            if constaints[job] >= players_to_affect {
                constaints[job] -= players_to_affect;
                self[job] = players_to_affect;
            } else {
                self[job] = constaints[job];
                constaints[job] = 0;
            };
        }
    }

    fn affect_yaks(&mut self, yak_count: usize) {
        // Equi-repartition
        let yaks_per_place = yak_count / WaitingPlace::COUNT;
        // Remainer (always less than free_spots_jobs count)
        let mut bonus_yaks = yak_count % WaitingPlace::COUNT;
        // For each job, add players and update constraints if needed
        for place in WaitingPlace::all() {
            // We add a bonus player if the remainer is not 0
            let yaks_to_affect = if bonus_yaks > 0 {
                bonus_yaks -= 1;
                yaks_per_place + 1
            } else {
                yaks_per_place
            };
            self[place] = yaks_to_affect;
        }
    }
}

impl GameCounts<Real> {
    pub const TYPE: &'static str = "GAME_COUNTS";

    pub fn with_player_count(player_count: usize) -> Self {
        Self {
            yaks_with_player: Default::default(),
            yaks_waiting: Default::default(),
            game_count_type: Real {
                player_count,
                desired_yak_count: player_count + player_count / 2,
                sheared_yak_count: 0,
                sampler: None,
            },
        }
    }
    pub fn reset(self) -> Self {
        let player_count = self.game_count_type.player_count;
        Self {
            yaks_with_player: Default::default(),
            yaks_waiting: Default::default(),
            game_count_type: Real {
                player_count,
                desired_yak_count: player_count + player_count / 2,
                sheared_yak_count: 0,
                sampler: None,
            },
        }
    }
    pub fn desired_yak_count(&self) -> usize {
        self.game_count_type.desired_yak_count
    }
    pub fn sheared_yak_count(&self) -> usize {
        self.game_count_type.sheared_yak_count
    }
    pub fn sampled(&self) -> bool {
        self.game_count_type.sampler.is_some_and(Sampler::sampled)
    }

    #[tracing::instrument(ret, level = "debug")]
    pub fn job_fees(self) -> JobFees {
        let Real {
            player_count,
            desired_yak_count,
            ..
        } = self.game_count_type;
        let constaints = self.get_constraints();

        let ideal_counts = GameCounts::new_ideal(player_count, desired_yak_count, constaints);

        let diffs = ideal_counts - self;

        RangedJobDiffs {
            diffs,
            player_count,
        }
        .into()
    }
    pub fn yak_counts(self) -> YakCounts {
        self.into()
    }

    #[tracing::instrument(ret, level = "debug")]
    fn get_constraints(&self) -> JobConstraints {
        let mut constraints = JobConstraints::default();

        // For each job constraints, add the current number of performer,
        // then add the number of yak available in the depedent place
        for job in Job::all() {
            let current_performers = self[job];
            let available_yaks = if let Some(place) = job.depends_on() {
                self[place]
            } else {
                // This is the first JOB!
                // It depends on the desired_yak_count and the number of yaks already alive
                let current_yak_count = self.total_yaks();
                self.game_count_type
                    .desired_yak_count
                    .saturating_sub(current_yak_count)
            };
            constraints[job] = current_performers + available_yaks;
        }

        constraints
    }

    #[tracing::instrument(ret, level = "debug", skip(client))]
    pub async fn with_updated_desired_yak_count(
        client: aws_sdk_dynamodb::Client,
        desired_yak_count: usize,
    ) -> Result<Self, Error> {
        Self::update_by_id(
            client,
            KeyId::NONE,
            Update::set("desired_yak_count", desired_yak_count),
        )
        .exists()
        .await
    }

    /// Decrease `from` by 1 and increase `to` by 1
    /// Update the GameCounts object directly and return a transaction
    /// item that will try and do the same in DynamoDB
    #[tracing::instrument(ret, level = "debug", skip(client))]
    pub async fn update_move_yak_with_sample_tracking(
        self,
        client: aws_sdk_dynamodb::Client,
        from: Option<GameCountIndex>,
        to: Option<GameCountIndex>,
    ) -> Result<Self, Error> {
        let (update, condition) = self.create_move_yak_update_and_condition(from, to);

        let update = update
            .map(|u| Update::combine([u, Self::create_sampling_tracking_update()]))
            .unwrap_or_else(Self::create_sampling_tracking_update);
        let condition = condition
            .map(|c| c & Self::exists())
            .unwrap_or(Self::exists());
        self.update(client, update)
            .condition(condition)
            .return_new()
            .await
    }

    fn create_sampling_tracking_update() -> Update<'static> {
        use std::time::SystemTime;
        Update::combine([
            Update::set(
                "update_ms",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            ),
            Update::set_custom("old_update_ms", UpdateSetRhs::if_not_exists("update_ms", 0)),
        ])
    }

    fn create_move_yak_update_and_condition(
        &self,
        from: Option<GameCountIndex>,
        to: Option<GameCountIndex>,
    ) -> (Option<Update<'static>>, Option<Condition<'static>>) {
        let from_update = from.map(|from| Update::decrement(from.to_string(), 1));
        let to_update = to
            .map(|to| Update::increment(to.to_string(), 1))
            .unwrap_or_else(|| Update::increment("sheared_yak_count", 1));
        let update = match (from_update, to) {
            (None, None) => None,
            (None, Some(_)) => Some(to_update),
            (Some(from), _) => Some(Update::combine([from, to_update])),
        };

        let condition = from.map(|from| Condition::gt(from.to_string(), 0));

        (update, condition)
    }
}

// impl Sub enables `ideal_counts - actual_counts` syntax.
// The diff tells us which jobs have too many/too few workers compared to the ideal distribution.
impl Sub<GameCounts<Real>> for GameCounts<Theorical> {
    type Output = JobDiffs;

    fn sub(self, ideal: GameCounts<Real>) -> Self::Output {
        let real = self;
        let mut diffs = JobDiffs::default();
        for job in Job::all() {
            diffs[job] = real[job] as i64 - ideal[job] as i64;
        }
        diffs
    }
}

// Singleton DynamoDB item: PK=SK="GAME_COUNTS" (like GameStatus, only one instance exists)
dynamodb_item! {
    #[table = MonoTable]
    GameCounts<Real> {
        #[partition_key]
        PK { const VALUE: &'static str = GameCounts::TYPE; }
        #[sort_key]
        SK { const VALUE: &'static str = GameCounts::TYPE; }
        ItemType { const VALUE: &'static str = GameCounts::TYPE; }
    }
}
