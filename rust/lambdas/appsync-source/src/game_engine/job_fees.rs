use std::ops::{Index, IndexMut};

use super::{GameCounts, Job, JobFees};

impl Default for JobFees {
    fn default() -> Self {
        Self::neutral()
    }
}

impl From<GameCounts> for JobFees {
    fn from(game_counts: GameCounts) -> Self {
        game_counts.job_fees()
    }
}

impl JobFees {
    // Neutral fee (100.0) is the baseline price when supply and demand are balanced.
    pub const NEUTRAL_FEE: f64 = 100.0;
    pub fn neutral() -> Self {
        Self {
            breeder: Self::NEUTRAL_FEE,
            driver: Self::NEUTRAL_FEE,
            shearer: Self::NEUTRAL_FEE,
        }
    }
    // Initial fees incentivize Breeders (200) since no yaks exist yet.
    // Driver and Shearer have no work to do at game start (fee = 0).
    pub fn initial() -> Self {
        Self {
            breeder: Self::NEUTRAL_FEE * 2.0,
            driver: 0.0,
            shearer: 0.0,
        }
    }
    // Fee adjustment: linear in the [-75, 75] range, exponential at extremes.
    // This creates strong incentives when supply/demand imbalance is severe.
    pub fn adjust_fee(&mut self, job: Job, incitation: f64) {
        fn extremities(mut incitation: f64) -> f64 {
            let is_neg = incitation <= -0.0;
            if is_neg {
                incitation = -incitation;
            }
            let res = 75.0 + 25.0 * (1.0 - 0.95f64.powf(incitation - 50.0));
            if is_neg {
                -res
            } else {
                res
            }
        }

        let adjustment = match incitation {
            -75.0..75.0 => incitation * 1.5,
            _ => extremities(incitation),
        };
        self[job] += adjustment * Self::NEUTRAL_FEE / 100.0;
    }
}

impl Index<Job> for JobFees {
    type Output = f64;

    fn index(&self, index: Job) -> &Self::Output {
        match index {
            Job::Breeder => &self.breeder,
            Job::Driver => &self.driver,
            Job::Shearer => &self.shearer,
        }
    }
}

impl IndexMut<Job> for JobFees {
    fn index_mut(&mut self, index: Job) -> &mut Self::Output {
        match index {
            Job::Breeder => &mut self.breeder,
            Job::Driver => &mut self.driver,
            Job::Shearer => &mut self.shearer,
        }
    }
}
