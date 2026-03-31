use std::collections::HashMap;

use dynamodb_facade::{AttributeValue, Item};
use lambda_appsync::ID;

use super::{Job, PlayerAssignment, Yak};

impl From<PlayerAssignment> for Yak {
    fn from(player_assignment: PlayerAssignment) -> Self {
        player_assignment.yak
    }
}

impl PlayerAssignment {
    pub fn new(job: Job, yak: Yak, fee: f64) -> Self {
        Self {
            job,
            yak: yak.into(),
            fee,
        }
    }

    // Converts to a DynamoDB Item using serde_dynamo.
    // The turbofish ::<_, HashMap<String, AttributeValue>> tells serde_dynamo
    // to serialize into the DynamoDB attribute map format.
    pub fn to_item(&self) -> Item {
        Item::from(
            serde_dynamo::to_item::<_, HashMap<String, AttributeValue>>(self)
                .expect("valid schema"),
        )
    }

    pub fn yak_id(&self) -> ID {
        self.yak.id
    }
}
