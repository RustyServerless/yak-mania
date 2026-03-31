use aws_sdk_dynamodb::{Client, Error};
use dynamodb_facade::{
    AttributeValue, Condition, DefaultMonoTable, DynamoDBItemOp, IntoAttributeValue, Item, ItemId,
    NoId,
};

use super::GameStatus;

/// Represents the game status in DynamoDB
/// Contains constants and implementations for DynamoDB storage
impl GameStatus {
    /// Name of attribute storing the actual game status value
    const PROPERTY_NAME: &'static str = "game_status";

    fn to_attribute_value(self) -> AttributeValue {
        self.to_string().into_attribute_value()
    }

    /// Atomically transitions the game to a new status, enforcing valid transitions.
    /// The game status can only transition in a specific order:
    /// Reset -> Started -> Stopped -> Reset
    /// Uses DynamoDB conditional expressions to prevent invalid transitions.
    pub async fn put_if_current_status_compatible(self, client: Client) -> Result<Self, Error> {
        let expected_value_condition = match self {
            // Only Stopped games can become Reset
            GameStatus::Reset => {
                Condition::eq(Self::PROPERTY_NAME, GameStatus::Stopped.to_string())
            }
            // Only Reset game, or no game status at all, can become Started
            GameStatus::Started => {
                Condition::not_exists(Self::PROPERTY_NAME)
                    | Condition::eq(Self::PROPERTY_NAME, GameStatus::Reset.to_string())
            }
            // Only Started games can become Stopped
            GameStatus::Stopped => {
                Condition::eq(Self::PROPERTY_NAME, GameStatus::Started.to_string())
            }
        };
        self.put(client)
            .condition(expected_value_condition)
            .send()
            .await?;
        Ok(self)
    }
}

// DynamoDBItem trait implementation: tells dynamodb-facade how to store/retrieve this type.
// GameStatus is a singleton item (NoId for both PK and SK), using TYPE as both PK and SK value.
impl<'a> dynamodb_facade::DynamoDBItem<'a> for GameStatus {
    type PkId = NoId;
    type SkId = NoId;
    type TableDefinition = DefaultMonoTable;
    const TYPE: &'static str = "GAME_STATUS";

    fn get_pk_from_id(_id: Self::PkId) -> dynamodb_facade::AttributeValue {
        Self::TYPE.into_attribute_value()
    }

    fn get_sk_from_id(_id: Self::SkId) -> dynamodb_facade::AttributeValue {
        Self::TYPE.into_attribute_value()
    }

    fn get_key(&self) -> dynamodb_facade::Item {
        Self::get_key_from_id(ItemId::NONE)
    }

    fn to_item(&self) -> Item {
        let mut item = self.to_item_core();
        item.insert(Self::PROPERTY_NAME.to_owned(), self.to_attribute_value());
        item
    }

    fn from_item(item: Item) -> Self {
        item.get(Self::PROPERTY_NAME)
            .and_then(|a| a.as_s().expect("valid schema").parse().ok())
            .expect("valid schema")
    }
}
