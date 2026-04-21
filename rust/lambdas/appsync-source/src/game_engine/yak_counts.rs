use std::ops::{Index, IndexMut};

use crate::game_engine::GameCounts;

use super::{Job, WaitingPlace, YakCounts};

// Converts internal GameCounts into the GraphQL-facing YakCounts type.
impl From<GameCounts> for YakCounts {
    fn from(game_counts: GameCounts) -> Self {
        let mut yak_counts = Self {
            // Nursery count is virtual: desired minus actual (how many yaks can still be bred).
            // saturating_sub prevents underflow (returns 0 instead of wrapping to a huge number).
            in_nursery: game_counts
                .desired_yak_count()
                .saturating_sub(game_counts.total_yaks()) as i32,
            total_sheared: game_counts.sheared_yak_count() as i32,
            ..Default::default()
        };

        for job in Job::all() {
            yak_counts[job] = game_counts[job] as i32;
        }
        for place in WaitingPlace::all() {
            yak_counts[place] = game_counts[place] as i32;
        }

        yak_counts
    }
}

impl Index<Job> for YakCounts {
    type Output = i32;

    fn index(&self, index: Job) -> &Self::Output {
        match index {
            Job::Breeder => &self.with_breeders,
            Job::Driver => &self.with_drivers,
            Job::Shearer => &self.with_shearers,
        }
    }
}

impl IndexMut<Job> for YakCounts {
    fn index_mut(&mut self, index: Job) -> &mut Self::Output {
        match index {
            Job::Breeder => &mut self.with_breeders,
            Job::Driver => &mut self.with_drivers,
            Job::Shearer => &mut self.with_shearers,
        }
    }
}

impl Index<WaitingPlace> for YakCounts {
    type Output = i32;

    fn index(&self, index: WaitingPlace) -> &Self::Output {
        match index {
            WaitingPlace::Warehouse => &self.in_warehouse,
            WaitingPlace::ShearingShed => &self.in_shearingshed,
        }
    }
}

impl IndexMut<WaitingPlace> for YakCounts {
    fn index_mut(&mut self, index: WaitingPlace) -> &mut Self::Output {
        match index {
            WaitingPlace::Warehouse => &mut self.in_warehouse,
            WaitingPlace::ShearingShed => &mut self.in_shearingshed,
        }
    }
}
