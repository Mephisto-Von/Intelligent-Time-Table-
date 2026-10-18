use std::sync::Arc;

use sqlx::SqlitePool;

use crate::notifications::Notifier;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jwt_secret: Arc<String>,
    pub notifier: Arc<Notifier>,
}

pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
