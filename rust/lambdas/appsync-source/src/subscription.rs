// FilterGroup from lambda-appsync: type-safe builder for AppSync Enhanced Subscription Filters.
// These let the server filter which updates each subscriber receives (instead of sending everything).
use lambda_appsync::subscription_filters::{FieldPath, FilterGroup};
use lambda_appsync::{AppsyncError, AppsyncEvent, AppsyncIdentity, appsync_operation};

use crate::Operation;

// `with_appsync_event`: tells lambda-appsync to pass the full AppSync event (including identity)
// to this handler, not just the arguments. Subscription handlers run once at subscribe time.
//
// Return type Option<FilterGroup>:
// - None = deliver ALL events to this subscriber (no filter)
// - Some(filter) = only deliver events matching the filter criteria
#[tracing::instrument(ret)]
#[appsync_operation(subscription(gameUpdated), with_appsync_event)]
pub async fn game_updated(
    event: &AppsyncEvent<Operation>,
) -> Result<Option<FilterGroup>, AppsyncError> {
    // Check if the subscriber is an Admin by inspecting their Cognito groups
    let ignored_sampled = match &event.identity {
        AppsyncIdentity::Cognito(appsync_identity_cognito) => appsync_identity_cognito
            .groups
            .iter()
            .flatten() // groups is Option<Vec<String>>, flatten handles the None case
            .any(|g| g == "Admins"),
        _ => false,
    };
    if ignored_sampled {
        // Admins receive ALL game updates (no filter)
        Ok(None)
    } else {
        // Regular players only get updates where sampled == true (~4 updates/sec)
        // to avoid overwhelming the WebSocket connection
        Ok(Some(FieldPath::new("sampled")?.eq(true).into()))
    }
}
