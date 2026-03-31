use dynamodb_facade::{
    AttributeValue, DefaultMonoTable, DynamoDBError, DynamoDBItem, DynamoDBItemOp,
    IntoAttributeValue, Item, ItemId,
};
use lambda_appsync::ID;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};

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
        client: aws_sdk_dynamodb::Client,
        from_place: WaitingPlace,
    ) -> Result<Option<Self>, DynamoDBError> {
        let mut yaks_in_place = Self::query(client, Self::pk_condition(from_place))
            .send()
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
impl<'a> DynamoDBItem<'a> for WaitingYak {
    type PkId = WaitingPlace;
    type SkId = ID;
    type TableDefinition = DefaultMonoTable;
    const TYPE: &'static str = Yak::TYPE;

    fn get_pk_from_id(id: Self::PkId) -> AttributeValue {
        format!("PLACE#{id}").into_attribute_value()
    }

    fn get_sk_from_id(id: Self::SkId) -> AttributeValue {
        format!("{}#{id}", Self::TYPE).into_attribute_value()
    }

    fn get_key(&self) -> Item {
        Self::get_key_from_id(ItemId::pk(self.location).sk(self.yak.id))
    }
}
