use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::notifications::{self, Recipient};
use crate::state::AppState;

/// Which notification channels are configured, and the events this install emits.
pub async fn notify_status(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let n = &st.notifier;
    Ok(Json(json!({
        "enabled": n.enabled(),
        "channels": n.channels(),
        "available_channels": ["knock", "webhook"],
        "events": [
            { "id": "cover-assigned", "label": "Substitute teacher assigned" },
            { "id": "leave-requested", "label": "Leave requested" },
            { "id": "leave-decided", "label": "Leave approved or rejected" },
        ],
    })))
}

/// Sends a test notification to the caller's own account.
pub async fn notify_test(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    user.require_admin()?;
    let recipient = Recipient {
        id: user.id.clone(),
        name: user.name.clone(),
        email: Some(user.email.clone()),
    };
    let delivered = st
        .notifier
        .notify(
            "test",
            std::slice::from_ref(&recipient),
            "TimetableMaster test notification",
            "If you can read this, notifications are configured correctly.",
            json!({ "kind": "test" }),
        )
        .await;
    let staff = notifications::staff_recipients(&st.pool, &user.school_id).await.len();
    Ok(Json(json!({
        "delivered": delivered,
        "enabled": st.notifier.enabled(),
        "staff_recipients": staff,
    })))
}
