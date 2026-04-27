use dynamodb_facade::{DynamoDBItemOp, Error, dynamodb_item};
use lambda_appsync::ID;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};

use crate::dynamodb_table::{ItemType, MonoTable, PK, SK};

use super::{WaitingPlace, Yak};

// WaitingYak: a Yak at a specific location (Warehouse or ShearingShed).
// #[serde(flatten)] merges yak fields at the same DynamoDB item level.
#[derive(Debug, Serialize, Deserialize)]
pub struct WaitingYak {
    #[serde(flatten)]
    yak: Yak,
    location: WaitingPlace,
}
impl From<WaitingYak> for Yak {
    fn from(waiting_yak: WaitingYak) -> Self {
        waiting_yak.yak
    }
}

impl WaitingYak {
    pub fn new(yak: Yak, location: WaitingPlace) -> Self {
        Self { yak, location }
    }
}

impl WaitingYak {
    pub fn location(&self) -> WaitingPlace {
        self.location
    }

    // Queries all yaks at a given place and picks one at random.
    // This spreads player activity across yaks instead of everyone grabbing the same one.
    pub async fn get_random_waiting(
        client: dynamodb_facade::Client,
        from_place: WaitingPlace,
    ) -> Result<Option<Self>, Error> {
        let mut yaks_in_place = Self::query(client, Self::key_condition(from_place))
            .all()
            .await?;

        Ok((0..yaks_in_place.len())
            .choose(&mut rand::rng())
            .map(|index| yaks_in_place.remove(index)))
    }
}

impl Yak {
    pub const TYPE: &'static str = "YAK";

    pub fn new() -> Self {
        Self { id: ID::new() }
    }
}

// DynamoDB mapping: PK = "PLACE#<location>", SK = "YAK#<uuid>"
// This enables querying all yaks at a specific place using just the PK.
dynamodb_item! {
    #[table = MonoTable]
    WaitingYak {
        #[sort_key]
        SK {
            fn attribute_id(&self) -> ID {
                self.yak.id
            }
            fn attribute_value(id) -> String {
                format!("{}#{id}", Yak::TYPE)
            }
        }
        #[partition_key]
        PK {
            fn attribute_id(&self) -> WaitingPlace {
                self.location
            }
            fn attribute_value(id) -> String {
                format!("PLACE#{id}")
            }
        }
        ItemType { const VALUE: &'static str = Yak::TYPE; }
    }
}
