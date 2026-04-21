use aws_sdk_dynamodb::Client;
use dynamodb_facade::{
    AttributeValue, Condition, DynamoDBItem, DynamoDBItemOp, Error, IntoAttributeValue, Item,
    attr_list, has_attributes,
};

use crate::dynamodb_table::{ItemType, MonoTable, PK, SK};

use super::GameStatus;

/// Represents the game status in DynamoDB
/// Contains constants and implementations for DynamoDB storage
impl GameStatus {
    pub const TYPE: &'static str = "GAME_STATUS";
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
        self.put(client).condition(expected_value_condition).await?;
        Ok(self)
    }
}

// DynamoDBItem trait implementation: tells dynamodb-facade how to store/retrieve this type.
// GameStatus is a singleton item (NoId for both PK and SK), using TYPE as both PK and SK value.
impl DynamoDBItem<MonoTable> for GameStatus {
    type AdditionalAttributes = attr_list!(ItemType);
    fn to_item(&self) -> dynamodb_facade::Item<MonoTable> {
        let minimal_item = dynamodb_facade::Item::minimal_from(self);
        minimal_item.with_attributes([(Self::PROPERTY_NAME.to_owned(), self.to_attribute_value())])
    }

    fn try_from_item(item: Item<MonoTable>) -> Result<Self, Error> {
        item.get(Self::PROPERTY_NAME)
            .ok_or_else(|| Error::custom("Invalid Schema"))
            .and_then(|a| {
                a.as_s()
                    .map_err(|e| Error::custom(format!("Invalid Schema: {e:?}")))
            })
            .and_then(|s| s.parse().map_err(Error::other))
    }
}
has_attributes! {
    GameStatus {
        PK {
            const VALUE: &'static str = GameStatus::TYPE;
        }

        SK {
            const VALUE: &'static str = GameStatus::TYPE;
        }

        ItemType {
            const VALUE: &'static str = GameStatus::TYPE;
        }
    }
}
