use std::ops::{Index, IndexMut};

use aws_sdk_dynamodb::types::TransactWriteItem;
use dynamodb_facade::{
    Condition, DynamoDBItemOp, DynamoDBItemTransactOp, Error, HasAttribute, HasConstAttribute,
    KeyId, NoId, Update, dynamodb_item,
};
use lambda_appsync::ID;
use serde::{Deserialize, Serialize};

use crate::dynamodb_table::{ItemType, MonoTable, PK, SK};

use super::{Job, JobFees, Player, PlayerAssignment, Yak};

// Index/IndexMut implementations enable player[Job::Breeder] syntax
// to access the job-specific counter fields on the Player struct.
impl Index<Job> for Player {
    type Output = i32;

    fn index(&self, index: Job) -> &Self::Output {
        match index {
            Job::Breeder => &self.yak_bred,
            Job::Driver => &self.yak_driven,
            Job::Shearer => &self.yak_sheared,
        }
    }
}

impl IndexMut<Job> for Player {
    fn index_mut(&mut self, index: Job) -> &mut Self::Output {
        match index {
            Job::Breeder => &mut self.yak_bred,
            Job::Driver => &mut self.yak_driven,
            Job::Shearer => &mut self.yak_sheared,
        }
    }
}

impl Player {
    pub const TYPE: &'static str = "PLAYER";

    pub fn start_new_assignment(&mut self, job: Job, yak: Yak, job_fees: JobFees) {
        let fee = job_fees[job];
        self.assignment = Some(PlayerAssignment::new(job, yak, fee));
    }

    pub fn finish_assignment(&mut self) -> Yak {
        let Some(assignment) = self.assignment.take() else {
            panic!("Player does not have an assignment");
        };

        // Update local balance
        self.balance += assignment.fee;
        assignment.into()
    }

    #[tracing::instrument(ret, level = "debug", skip(client))]
    pub async fn update_with_secret_check(
        client: aws_sdk_dynamodb::Client,
        key_id: KeyId<ID, NoId>,
        update: Update<'_>,
        secret: String,
    ) -> Result<Self, Error> {
        Player::update_by_id(client, key_id, update)
            .condition(Condition::eq("secret", secret))
            .await
    }

    /// Update the Job of the player
    /// Update the Player object directly and return a transaction
    /// item that will try and do the same in DynamoDB
    #[tracing::instrument(ret, level = "debug")]
    pub fn transact_start_assignment_with_secret_check(&self, secret: String) -> TransactWriteItem {
        self.transact_update(Update::set(
            "assignment",
            self.assignment
                .as_ref()
                .expect("Player should have an assignment")
                .to_attribute_value(),
        ))
        .condition(Condition::eq("secret", secret) & Condition::not_exists("assignment"))
        .build()
    }
    #[tracing::instrument(ret, level = "debug")]
    pub fn transact_finish_assignment_with_secret_check(
        &self,
        secret: String,
    ) -> TransactWriteItem {
        let assignment = self
            .assignment
            .as_ref()
            .expect("Player should have an assignment");
        let yak_count_field = match assignment.job {
            Job::Breeder => "yak_bred",
            Job::Driver => "yak_driven",
            Job::Shearer => "yak_sheared",
        };

        let old_balance = self.balance;
        let new_balance = old_balance + assignment.fee;

        self.transact_update(Update::combine([
            Update::set("balance", new_balance),
            Update::increment(yak_count_field, 1),
            Update::remove("assignment"),
        ]))
        .condition(
            Condition::eq("secret", secret)
                & Condition::eq("assignment", assignment.to_attribute_value())
                & Condition::eq("balance", old_balance),
        )
        .build()
    }
}

// DynamoDB mapping: PK = "PLAYER#<uuid>", SK = "PLAYER"

// impl DynamoDBItem<MonoTable> for Player {
//     type AdditionalAttributes = attr_list!(ItemType);
// }
// impl HasAttribute<PK> for Player {
//     type Id<'id> = ID;
//     type V = String;
//     fn attribute_value(id: Self::Id<'_>) -> Self::V {
//         format!("{}#{id}", Self::TYPE)
//     }
//     fn attribute_id(&self) -> Self::Id<'_> {
//         self.id
//     }
// }
// impl HasConstAttribute<SK> for Player {
//     type V = &'static str;
//     const VALUE: Self::V = Self::TYPE;
// }
// impl HasConstAttribute<ItemType> for Player {
//     type V = &'static str;
//     const VALUE: Self::V = Self::TYPE;
// }
dynamodb_item! {
    #[table = MonoTable]
    Player {
        #[partition_key]
        PK {
            fn attribute_id(&self) -> ID {
                self.id
            }
            fn attribute_value(id) -> String {
                format!("{}#{id}", Self::TYPE)
            }
        }

        #[sort_key]
        SK {
            const VALUE: &'static str = Self::TYPE;
        }

        ItemType {
            const VALUE: &'static str = Self::TYPE;
        }
    }
}

// PlayerWithSecret wraps Player with a secret field for authentication.
// #[serde(flatten)] merges the Player fields at the same JSON/DynamoDB level (no nesting).
// This type shares the same DynamoDB key as Player (same PK/SK/TYPE).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerWithSecret {
    #[serde(flatten)]
    player: Player,
    secret: String,
}
impl PlayerWithSecret {
    pub fn new(name: String, secret: String) -> Self {
        Self {
            player: Player {
                name,
                ..Default::default()
            },
            secret,
        }
    }
    pub fn reset(self) -> Self {
        Self {
            player: Player {
                id: self.player.id,
                name: self.player.name,
                ..Default::default()
            },
            secret: self.secret,
        }
    }

    pub fn into_player(self) -> Player {
        self.player
    }
}

dynamodb_item! {
    #[table = MonoTable]
    PlayerWithSecret {
        #[partition_key]
        PK {
            fn attribute_id(&self) -> <Player as HasAttribute<PK>>::Id<'id> {
                <Player as HasAttribute<PK>>::attribute_id(&self.player)
            }
            fn attribute_value(id) -> <Player as HasAttribute<PK>>::Value {
                <Player as HasAttribute<PK>>::attribute_value(id)
            }
        }
        #[sort_key]
        SK { const VALUE: &'static str = <Player as HasConstAttribute<ItemType>>::VALUE; }
        ItemType { const VALUE: &'static str = <Player as HasConstAttribute<ItemType>>::VALUE; }
    }
}
