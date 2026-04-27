use dynamodb_facade::IntoAttributeValue;
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

    pub fn yak_id(&self) -> ID {
        self.yak.id
    }
}

impl IntoAttributeValue for &PlayerAssignment {
    fn into_attribute_value(self) -> dynamodb_facade::AttributeValue {
        serde_dynamo::to_attribute_value(self).expect("is valid for serialization")
    }
}
