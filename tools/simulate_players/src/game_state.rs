#![allow(unused)]

use std::ops::Index;

lambda_appsync::make_types!("graphql/schema.gql", derive = YakCounts: Copy, derive = JobFees: Copy);

impl YakCounts {
    /// Returns true if any yaks are still being processed by players.
    pub fn any_in_progress(&self) -> bool {
        self.with_breeders > 0 || self.with_drivers > 0 || self.with_shearers > 0
    }

    pub fn max_reached(&self) -> bool {
        self.in_nursery == 0
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
