mod game_status;
mod game_update;
mod internal;
mod job;
mod job_fees;
mod player;
mod player_assignment;
mod yak;
mod yak_counts;

// This macro reads the GraphQL schema at **compile time** and generates Rust structs/enums
// for every type, input, and enum defined in the schema (Player, GameStatus, GameUpdate, etc.).
// The `derive = Type: Trait` syntax adds extra derive macros to specific generated types.
//
// These types are then extended with custom implementations in the submodules above
// (e.g. game_status.rs adds DynamoDB serialization to the generated GameStatus enum).
//
// See: https://docs.rs/lambda-appsync/latest/lambda_appsync/macro.make_types.html
lambda_appsync::make_types!(
    "graphql/schema.gql",
    derive = GameStatus: Default,
    derive = GameState: Default,
    derive = GameUpdate: Default,
    derive = YakCounts: Default,
    derive = Player: Default,
);

// Re-export internal types that operations.rs needs
pub use internal::{GameCountIndex, GameCounts};
pub use player::PlayerWithSecret;
pub use yak::WaitingYak;
