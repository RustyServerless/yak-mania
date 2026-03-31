// awssdk-instrumentation provides SpanWrite trait for setting OTel/X-Ray attributes on spans,
// and DefaultInstrumentor for accessing the root Lambda invocation span.
use awssdk_instrumentation::{
    lambda::layer::{DefaultInstrumentor, Instrumentor},
    span_write::{SpanWrite, Status},
};
use lambda_appsync::AppsyncError;

// Creates an AppsyncError for server-side failures (5xx) and marks both the current span
// and the root invocation span as errored in X-Ray.
pub fn internal_server_error<E: core::fmt::Display + core::fmt::Debug>(error: E) -> AppsyncError {
    // The HTTP status code here only serves so that the OTel->XRay translator correctly
    // sets the fault/error flag on the X-Ray segment.
    // So the exact code does not matter beyond the range (400-499 = error, 500-599 = fault).
    const CODE: u16 = 500;
    const ERROR_TYPE: &str = "ServerError";
    let appsync_error = AppsyncError::new(ERROR_TYPE, "Internal Server Error");
    tracing::error!(?error);
    // set_http_status_code() is from the SpanWrite trait (awssdk-instrumentation).
    // It sets an attribute on the current tracing span that the OTel->XRay translator reads.
    tracing::Span::current().set_http_status_code(CODE);
    // We make the choice to mark the entire Batch invocation as failed
    // if one of the request fail.
    // Note that it does not change anything to the response sent to requesters of the API, it
    // just drives how X-Ray will show the segment in its UI
    // DefaultInstrumentor::with_invocation_span() accesses the ROOT Lambda invocation span
    // (not the per-event span), allowing us to mark the entire invocation as errored.
    DefaultInstrumentor::with_invocation_span(|span| {
        span.set_status(Status::Error {
            description: ERROR_TYPE.into(),
        });
        span.set_http_status_code(CODE);
    });
    appsync_error
}

// Declarative macro for client-facing errors (400-level).
// Sets both the current span and the root invocation span to error status in X-Ray.
#[macro_export]
macro_rules! trace_appsync_error {
    ($error_type:expr, $error_message:expr $(,)?) => {
        {
            // 400 = client error (X-Ray shows as "error" vs 500 = "fault")
            const CODE: u16 = 400;
            use lambda_appsync::AppsyncError;
            use awssdk_instrumentation::{span_write::{SpanWrite, Status}, lambda::layer::{DefaultInstrumentor, Instrumentor}};
            let appsync_error = AppsyncError::new($error_type, $error_message);
            tracing::error!(exception.type = &appsync_error.error_type, exception.message = &appsync_error.error_message);
            tracing::Span::current().set_http_status_code(CODE);
            DefaultInstrumentor::with_invocation_span(|span| {
                span.set_status(Status::Error {
                    description: appsync_error.error_type.clone().into(),
                });
                span.set_http_status_code(CODE);
            });
            appsync_error
        }
    };
}
