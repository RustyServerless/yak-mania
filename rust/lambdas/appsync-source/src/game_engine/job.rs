use super::{Job, WaitingPlace};

impl Job {
    // The yak pipeline: Breeder -> Warehouse -> Driver -> ShearingShed -> Shearer
    // Each job depends on a WaitingPlace where yaks arrive from the previous step.
    // Breeder has no dependency (creates baby yaks from thin air).
    pub fn depends_on(self) -> Option<WaitingPlace> {
        match self {
            Job::Breeder => None,
            Job::Driver => Some(WaitingPlace::Warehouse),
            Job::Shearer => Some(WaitingPlace::ShearingShed),
        }
    }
}
