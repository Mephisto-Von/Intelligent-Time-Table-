mod audit;
mod auth;
mod dates;
mod db;
mod error;
mod ical;
mod models;
mod notifications;
mod pdf;
mod routes;
mod seed;
mod solver;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=warn")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let seed_demo_flag = args.iter().any(|a| a == "--seed-demo");

    let db_path = std::env::var("TIMETABLE_DB").unwrap_or_else(|_| "data/timetable.db".to_string());
    let pool = db::connect(&db_path).await?;
    let jwt_secret = load_or_create_secret()?;
    let notifier = notifications::Notifier::from_env();
    if notifier.enabled() {
        tracing::info!("notifications enabled via: {}", notifier.channels().join(", "));
    } else {
        tracing::info!("notifications disabled (set NOTIFY_WEBHOOK_URL or KNOCK_API_KEY to enable)");
    }
    let state = AppState {
        pool,
        jwt_secret: Arc::new(jwt_secret),
        notifier: Arc::new(notifier),
    };

    if seed_demo_flag {
        match seed::ensure_demo_account(&state).await {
            Ok((_, _, created)) => {
                let verb = if created { "created" } else { "already present" };
                tracing::info!(
                    "demo school {verb}: login {} / {}",
                    seed::DEMO_EMAIL,
                    seed::DEMO_PASSWORD
                );
            }
            Err(e) => tracing::error!("demo seed failed: {e}"),
        }
    }

    let mut app = routes::api_router()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let dist = std::path::Path::new("../frontend/dist");
    if dist.join("index.html").exists() {
        tracing::info!("serving frontend from {}", dist.display());
        app = app.fallback_service(
            ServeDir::new(dist).fallback(ServeFile::new(dist.join("index.html"))),
        );
    } else {
        tracing::info!("frontend build not found at {}; API only", dist.display());
    }

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("intelligent timetable listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

fn load_or_create_secret() -> anyhow::Result<String> {
    if let Ok(s) = std::env::var("TIMETABLE_JWT_SECRET") {
        if s.len() >= 16 {
            return Ok(s);
        }
    }
    let path = std::path::Path::new("data/jwt_secret.txt");
    if let Ok(existing) = std::fs::read_to_string(path) {
        let existing = existing.trim().to_string();
        if existing.len() >= 16 {
            return Ok(existing);
        }
    }
    std::fs::create_dir_all("data")?;
    let secret: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..48).map(|_| format!("{:02x}", rng.gen_range(0u8..=255u8))).collect()
    };
    std::fs::write(path, &secret)?;
    Ok(secret)
}
