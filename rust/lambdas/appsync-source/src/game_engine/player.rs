use std::ops::{Index, IndexMut};

use aws_sdk_dynamodb::types::TransactWriteItem;
use dynamodb_facade::{
    AttributeValue, Condition, DefaultMonoTable, DynamoDBItem, DynamoDBItemOp,
    DynamoDBItemTransactOp, IntoAttributeValue, Item, ItemId, NoId, Update,
};
use lambda_appsync::ID;
use serde::{Deserialize, Serialize};

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
        item_id: ItemId<
            <Self as DynamoDBItem<'static>>::PkId,
            <Self as DynamoDBItem<'static>>::SkId,
        >,
        update: Update<'_>,
        secret: String,
    ) -> Result<Self, aws_sdk_dynamodb::Error> {
        Ok(Player::update_by_id(client, item_id, update)
            .condition(Condition::eq("secret", secret))
            .send()
            .await?)
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
                .to_item(),
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
                & Condition::eq("assignment", assignment.to_item())
                & Condition::eq("balance", old_balance),
        )
        .build()
    }
}

// DynamoDB mapping: PK = "PLAYER#<uuid>", SK = "PLAYER"
impl<'a> DynamoDBItem<'a> for Player {
    type PkId = ID;
    type SkId = NoId;
    type TableDefinition = DefaultMonoTable;
    const TYPE: &'static str = "PLAYER";

    fn get_pk_from_id(id: Self::PkId) -> AttributeValue {
        format!("{}#{id}", Self::TYPE).into_attribute_value()
    }

    fn get_sk_from_id(_id: Self::SkId) -> AttributeValue {
        Self::TYPE.into_attribute_value()
    }

    fn get_key(&self) -> Item {
        Self::get_key_from_id(ItemId::pk(self.id))
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

impl<'a> DynamoDBItem<'a> for PlayerWithSecret {
    type PkId = <Player as DynamoDBItem<'a>>::PkId;
    type SkId = <Player as DynamoDBItem<'a>>::SkId;
    type TableDefinition = <Player as DynamoDBItem<'a>>::TableDefinition;
    const TYPE: &'static str = Player::TYPE;

    fn get_pk_from_id(id: Self::PkId) -> AttributeValue {
        Player::get_pk_from_id(id)
    }

    fn get_sk_from_id(id: Self::SkId) -> AttributeValue {
        Player::get_sk_from_id(id)
    }

    fn get_key(&self) -> Item {
        self.player.get_key()
    }
}
