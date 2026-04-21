use dynamodb_facade::AttributeValue;
use lambda_appsync::ID;

use super::{Job, PlayerAssignment, Yak};

impl From<PlayerAssignment> for Yak {
    fn from(player_assignment: PlayerAssignment) -> Self {
        player_assignment.yak
    }
}

impl PlayerAssignment {
    pub fn new(job: Job, yak: Yak, fee: f64) -> Self {
        Self { job, yak, fee }
    }

    pub fn to_attribute_value(&self) -> AttributeValue {
        serde_dynamo::to_attribute_value(self).expect("is valid for serialization")
    }

    pub fn yak_id(&self) -> ID {
        self.yak.id
    }
}
