//! The Croncave control plane: accounts, computers, the scheduler and run queue, events,
//! the ledger, the AI gateway, and the relay that carries everything to computers.
//!
//! In the prototype the relay and the preview edge run in the same process as the
//! control plane (see docs/decisions.md); they talk through the same interfaces they
//! would across a network.

pub mod accounts;
pub mod admin;
pub mod api;
pub mod apps;
pub mod assistant;
pub mod auth;
pub mod billing;
pub mod catalog;
pub mod clock;
pub mod code;
pub mod computers;
pub mod config;
pub mod crypto;
pub mod demo;
pub mod dev;
pub mod error;
pub mod events;
pub mod explain;
pub mod files_api;
pub mod home;
pub mod jobs;
pub mod ledger;
pub mod measure;
pub mod model;
pub mod notify_worker;
pub mod orchestrator;
pub mod preview_edge;
pub mod providers;
pub mod relay_hooks;
pub mod scripts;
pub mod seed;
pub mod state;
pub mod watcher;

use sqlx::postgres::PgPoolOptions;

pub use state::AppState;

/// Connect, migrate, seed, and build the shared state.
pub async fn build(cfg: config::Config) -> anyhow::Result<AppState> {
    let db = PgPoolOptions::new().max_connections(30).connect(&cfg.database_url).await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    seed::run(&db).await?;
    state::App::new(cfg, db)
}

/// Start the background work: live updates, orchestrator, scheduler, billing, notifications.
pub fn spawn_workers(app: &AppState) {
    tokio::spawn(events::listen(app.clone()));
    tokio::spawn(orchestrator::run_loop(app.clone()));
    tokio::spawn(jobs::run_loops(app.clone()));
    tokio::spawn(billing::run_loop(app.clone()));
    tokio::spawn(notify_worker::run_loop(app.clone()));
}

/// Run everything until the process is stopped.
pub async fn serve(cfg: config::Config) -> anyhow::Result<()> {
    let api_addr = cfg.api_addr;
    let preview_addr = cfg.preview_addr;
    let app = build(cfg).await?;
    spawn_workers(&app);
    let api = tokio::net::TcpListener::bind(api_addr).await?;
    let previews = tokio::net::TcpListener::bind(preview_addr).await?;
    tracing::info!(%api_addr, %preview_addr, driver = app.providers.compute.name(), "Croncave control plane listening");
    let a = axum::serve(api, api::router(app.clone()).layer(tower_http::trace::TraceLayer::new_for_http()));
    let p = axum::serve(previews, preview_edge::router(app.clone()));
    tokio::try_join!(async { a.await }, async { p.await })?;
    Ok(())
}
