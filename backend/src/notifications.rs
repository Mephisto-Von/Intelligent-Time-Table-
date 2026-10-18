//! Best-effort outbound notifications.
//!
//! Two opt-in channels, configured entirely by environment variables:
//!   * `NOTIFY_WEBHOOK_URL` — POSTs a JSON payload to any webhook (Slack, Discord,
//!     a Zapier/Make hook, or a local listener during development).
//!   * `KNOCK_API_KEY`     — sends through Knock (<https://knock.app>), which in turn
//!     delivers email/in-app/push to the recipients' addresses.
//!
//! When neither is configured the notifier is a no-op, so the application never
//! requires notification credentials to run. Delivery is always best-effort:
//! failures are logged and never fail the originating request.

use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

/// A notification target resolved to a platform user (with an email when known).
#[derive(Debug, Clone, Serialize)]
pub struct Recipient {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
}

#[derive(Clone)]
pub struct Notifier {
    http: reqwest::Client,
    webhook_url: Option<String>,
    knock_key: Option<String>,
}

impl Notifier {
    pub fn from_env() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        let clean = |k: &str| {
            std::env::var(k)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        Self {
            http,
            webhook_url: clean("NOTIFY_WEBHOOK_URL"),
            knock_key: clean("KNOCK_API_KEY"),
        }
    }

    pub fn channels(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.knock_key.is_some() {
            out.push("knock");
        }
        if self.webhook_url.is_some() {
            out.push("webhook");
        }
        out
    }

    pub fn enabled(&self) -> bool {
        self.knock_key.is_some() || self.webhook_url.is_some()
    }

    /// Sends one notification to every recipient over every configured channel.
    /// Returns the list of channels that accepted the message.
    pub async fn notify(
        &self,
        event: &str,
        recipients: &[Recipient],
        subject: &str,
        body: &str,
        data: Value,
    ) -> Vec<String> {
        let mut delivered = Vec::new();
        if !self.enabled() {
            tracing::debug!("notification '{event}' skipped: no channel configured");
            return delivered;
        }

        if let Some(url) = &self.webhook_url {
            let payload = json!({
                "event": event,
                "subject": subject,
                "body": body,
                "recipients": recipients,
                "data": data,
            });
            match self.http.post(url).json(&payload).send().await {
                Ok(r) if r.status().is_success() => delivered.push("webhook".to_string()),
                Ok(r) => tracing::warn!("webhook notification '{event}' returned {}", r.status()),
                Err(e) => tracing::warn!("webhook notification '{event}' failed: {e}"),
            }
        }

        if let Some(key) = &self.knock_key {
            let targets: Vec<Value> = recipients
                .iter()
                .map(|r| json!({ "id": r.id, "name": r.name, "email": r.email }))
                .collect();
            let payload = json!({
                "trigger": event,
                "recipients": targets,
                "data": { "subject": subject, "body": body, "detail": data },
            });
            match self
                .http
                .post("https://api.knock.app/v1/notify")
                .bearer_auth(key)
                .json(&payload)
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => delivered.push("knock".to_string()),
                Ok(r) => tracing::warn!("knock notification '{event}' returned {}", r.status()),
                Err(e) => tracing::warn!("knock notification '{event}' failed: {e}"),
            }
        }

        delivered
    }
}

/// Who to notify about a teacher: their linked platform accounts when they have
/// them, otherwise the teacher record's own email address.
pub async fn recipients_for_teacher(
    pool: &SqlitePool,
    school_id: &str,
    teacher_id: &str,
) -> Vec<Recipient> {
    let rows = sqlx::query(
        "SELECT id, name, email FROM users WHERE school_id = ? AND teacher_id = ?",
    )
    .bind(school_id)
    .bind(teacher_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    if !rows.is_empty() {
        return rows
            .iter()
            .map(|r| Recipient {
                id: r.get("id"),
                name: r.get("name"),
                email: Some(r.get::<String, _>("email")),
            })
            .collect();
    }

    // No portal account: fall back to the teacher's contact email.
    match sqlx::query("SELECT id, name, email FROM teachers WHERE id = ? AND school_id = ?")
        .bind(teacher_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(r)) => {
            let email: Option<String> = r.get("email");
            match email.filter(|e| !e.trim().is_empty()) {
                Some(email) => vec![Recipient {
                    id: r.get("id"),
                    name: r.get("name"),
                    email: Some(email),
                }],
                None => vec![],
            }
        }
        _ => vec![],
    }
}

/// Every admin/scheduler account in the school (used for leave requests).
pub async fn staff_recipients(pool: &SqlitePool, school_id: &str) -> Vec<Recipient> {
    let rows = sqlx::query(
        "SELECT id, name, email FROM users WHERE school_id = ? AND role IN ('admin', 'scheduler')",
    )
    .bind(school_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.iter()
        .map(|r| Recipient {
            id: r.get("id"),
            name: r.get("name"),
            email: Some(r.get::<String, _>("email")),
        })
        .collect()
}
