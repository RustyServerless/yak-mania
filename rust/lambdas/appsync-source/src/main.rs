mod dynamodb_table;
mod error_utils;
mod game_engine;
mod operations;
mod subscription;

// This macro reads the GraphQL schema file at **compile time** and generates:
// - An `Operation` enum with one variant per GraphQL operation (query, mutation, subscription)
// - Deserialization logic from AppSync event payloads
// - Routing logic to dispatch each operation to its handler function
//
// `type_module = game_engine` tells the macro to look for generated types (Player, GameUpdate, etc.)
// in the `game_engine` module instead of the current scope.
// `error_logging = false` disables the crate's default error logging because
// this project uses custom error logging in `error_utils.rs`.
//
// See: https://docs.rs/lambda-appsync/latest/lambda_appsync/macro.make_operation.html
lambda_appsync::make_operation!(
    "graphql/schema.gql",
    type_module = game_engine,
    error_logging = false
);

// This macro generates a `Handlers` trait with:
// - `appsync_handler()`: handles a single AppSync event
// - `appsync_batch_handler()`: handles a batch of events (used when MaxBatchSize > 1)
// - `service_fn()`: wraps the batch handler as a Lambda service function
// The trait dispatches each `Operation` variant to the function annotated
// with #[appsync_operation(...)].
//
// See: https://docs.rs/lambda-appsync/latest/lambda_appsync/macro.make_handlers.html
lambda_appsync::make_handlers!();

// Override the default batch handler to add per-event OpenTelemetry tracing.
// Each event in the batch is spawned as a separate async task with its own tracing span.
struct InstrumentedHandlers;
use lambda_appsync::{AppsyncEvent, AppsyncResponse};
impl Handlers for InstrumentedHandlers {
    // AppSync sends batched requests (up to MaxBatchSize from the resolver config) as a Vec.
    // The handler must return responses in the same order.
    async fn appsync_batch_handler(events: Vec<AppsyncEvent<Operation>>) -> Vec<AppsyncResponse> {
        // DefaultInstrumentor::spawn() from awssdk-instrumentation creates a traced async task
        // that propagates OTel context (standard tokio::spawn would lose the context).
        use awssdk_instrumentation::lambda::layer::{DefaultInstrumentor, Instrumentor};
        // .instrument(span) from the tracing crate attaches a span to a Future
        use tracing::Instrument;
        let handles = events
            .into_iter()
            .enumerate()
            .map(|(index, event)| {
                let operation = event.info.operation;
                // Spawn each handler as a separate traced task:
                // - info_span! creates a span with the operation name and batch index
                // - "otel.name" is a special OTel attribute that sets the span's display name
                //   in tracing UIs (X-Ray, Jaeger, etc.)
                DefaultInstrumentor::spawn(Self::appsync_handler(event).instrument(
                    tracing::info_span!(
                        "AppsyncEvent",
                        "otel.name" = format!("AppsyncEvent #{index}"),
                        batch_index = index,
                        ?operation
                    ),
                ))
            })
            .collect::<Vec<_>>();
        // Await sequentially to maintain response order
        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap())
        }
        results
    }
}

// This macro from `awssdk-instrumentation` generates the entire main() function:
// - Initializes the OTel tracer provider with X-Ray exporter
// - Sets up tracing-subscriber with JSON console output and OTel bridge
// - Loads AWS SDK config from environment
// - Creates an instrumented DynamoDB client (with OTel tracing of SDK calls)
//   accessible via a `dynamodb()` function anywhere in the crate
// - Wraps the Lambda runtime with the OTel Tower layer (per-invocation spans, flush on drop)
// - Starts the Lambda runtime
//
// See: https://docs.rs/awssdk-instrumentation/latest/awssdk_instrumentation/macro.make_lambda_runtime.html
awssdk_instrumentation::make_lambda_runtime!(InstrumentedHandlers::service_fn, dynamodb() -> dynamodb_facade::Client);
