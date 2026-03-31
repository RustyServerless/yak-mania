// This is the BASELINE Lambda: no `awssdk-instrumentation`, no OTel.
// Compare with instrumentation-demo/src/main.rs to see what the crate replaces.
// The handler code is intentionally identical in both — instrumentation is transparent to business logic.

use ::std::sync::OnceLock;

use aws_lambda_events::{
    event::{sns::SnsMessage, sqs::SqsBatchResponse},
    sqs::SqsEventObj,
};
use aws_sdk_dynamodb::types::AttributeValue;
use tracing_subscriber::EnvFilter;

#[tracing::instrument(skip_all)]
async fn handler(
    event: lambda_runtime::LambdaEvent<serde_json::Value>,
) -> Result<SqsBatchResponse, lambda_runtime::Error> {
    println!("{}", event.payload.to_string());

    // SqsEventObj<SnsMessage>: the event is an SNS message wrapped in an SQS envelope
    // (because the architecture is SNS -> SQS -> Lambda)
    let sqs_payload: SqsEventObj<SnsMessage> =
        serde_json::from_value(event.payload).expect("should be sqs event payload");

    // SqsBatchResponse supports partial batch failure reporting:
    // only failed message IDs are reported back to SQS for retry.
    let mut response = SqsBatchResponse::default();

    for record in sqs_payload.records {
        let sns_message_id = record.body.message_id;
        tracing::info!(%sns_message_id, "Processing SNS message");
        let table_name = std::env::var("TABLE_NAME").expect("TABLE_NAME must be set");
        let lambda_name = std::env::var("LAMBDA_NAME").expect("LAMBDA_NAME must be set");
        match dynamodb()
            .put_item()
            .table_name(table_name)
            .item("PK", AttributeValue::S(sns_message_id))
            .item("SK", AttributeValue::S(lambda_name))
            .send()
            .await
        {
            Ok(_) => {}
            Err(error) => {
                tracing::error!(%error, "Failed to process message");
                if let Some(sqs_message_id) = record.message_id {
                    response.add_failure(sqs_message_id);
                }
            }
        }
    }
    Ok(response)
}

// --- Manual boilerplate that awssdk-instrumentation::make_lambda_runtime! replaces ---

// OnceLock provides thread-safe, one-time initialization for the AWS SDK config.
// This is the standard pattern for Lambda: initialize once on cold start, reuse across invocations.
static AWS_SDK_CONFIG: OnceLock<::aws_config::SdkConfig> = OnceLock::new();
fn aws_sdk_config() -> &'static ::aws_config::SdkConfig {
    AWS_SDK_CONFIG.get().unwrap()
}
async fn sdk_config_init() {
    AWS_SDK_CONFIG
        .set(aws_config::load_from_env().await)
        .unwrap();
}

// Plain DynamoDB client — no OTel interceptor, so DynamoDB calls won't appear in X-Ray traces.
fn dynamodb() -> aws_sdk_dynamodb::Client {
    static CLIENT: OnceLock<aws_sdk_dynamodb::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| aws_sdk_dynamodb::Client::new(aws_sdk_config()))
        .clone()
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    // Manual tracing subscriber setup with JSON output for CloudWatch
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env())
        // this needs to be set to remove duplicated information in the log.
        .with_current_span(false)
        // this needs to be set to false, otherwise ANSI color codes will
        // show up in a confusing manner in CloudWatch logs.
        .with_ansi(false)
        // remove the name of the function from every log entry
        .with_target(false)
        .init();

    sdk_config_init().await;

    lambda_runtime::Runtime::new(lambda_runtime::service_fn(handler))
        .run()
        .await
}
