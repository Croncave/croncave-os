//! Every route of the control plane.

use axum::Router;
use axum::routing::{delete, get, post, put};

use crate::state::AppState;
use crate::*;

/// What computers may reach on the extra relay address: the relay and the demo sites,
/// never the API.
pub fn relay_router(app: AppState) -> Router {
    Router::new()
        .route("/demo/listings", get(demo::listings))
        .route("/demo/page", get(demo::page))
        .with_state(app.clone())
        .merge(app.relay.router())
}

pub fn router(app: AppState) -> Router {
    let mut api = Router::new()
        // Sign-in and the person
        .route("/auth/email", post(auth::send_link))
        .route("/auth/verify", post(auth::verify_link))
        .route("/auth/phone", post(auth::send_code))
        .route("/auth/phone/verify", post(auth::verify_code))
        .route("/auth/signout", post(auth::sign_out))
        .route("/me", get(accounts::me))
        .route("/me/prefs", post(accounts::update_prefs))
        .route("/me/audit", get(accounts::audit_log))
        .route("/schemes/{id}", get(accounts::scheme))
        .route("/time-zones", get(accounts::time_zones))
        .route("/config", get(accounts::public_config))
        // Plans and billing
        .route("/plans", get(billing::plans))
        .route("/signup/plan", post(billing::choose_plan))
        .route("/signup/trial", get(billing::get_trial_offer).post(billing::answer_trial))
        .route("/billing", get(billing::summary))
        .route("/billing/cap", post(billing::set_cap))
        .route("/billing/plan", post(billing::change_plan))
        .route("/billing/promo", post(billing::redeem_promo))
        .route("/billing/invoices/{id}", get(billing::invoice))
        // Home and Activity
        .route("/home", get(home::home))
        .route("/events", get(events::list))
        .route("/events/read", post(events::mark_read))
        .route("/live", get(events::sse))
        .route("/apps", get(apps::list))
        // Computers
        .route("/computers", post(computers::create))
        .route("/computers/{id}", get(computers::get).patch(computers::update).delete(computers::delete))
        .route("/computers/{id}/action", post(computers::action))
        .route("/computers/{id}/presence", post(computers::presence))
        .route("/computers/{id}/jobs", get(jobs::list))
        .route("/computers/{id}/runs", get(jobs::computer_runs))
        // Files
        .route("/computers/{id}/files", get(files_api::list))
        .route("/computers/{id}/files/preview", get(files_api::preview))
        .route("/computers/{id}/files/download", get(files_api::download))
        .route("/computers/{id}/files/write", put(files_api::write))
        .route("/computers/{id}/files/mkdir", post(files_api::mkdir))
        .route("/computers/{id}/files/move", post(files_api::rename))
        .route("/computers/{id}/files/delete", post(files_api::delete))
        .route("/computers/{id}/files/trash", get(files_api::trash))
        .route("/computers/{id}/files/restore", post(files_api::restore))
        .route("/computers/{id}/files/empty-trash", post(files_api::empty_trash))
        .route("/computers/{id}/files/usage", get(files_api::usage))
        .route("/computers/{id}/files/recent", get(files_api::recent))
        .route("/computers/{id}/files/copy", post(files_api::copy_to))
        .route("/computers/{id}/uploads", post(files_api::start_upload))
        .route("/computers/{id}/uploads/{upload}", get(files_api::upload_status).put(files_api::upload_chunk))
        .route("/computers/{id}/uploads/{upload}/finish", post(files_api::finish_upload))
        // Jobs and runs (every app)
        .route("/jobs/{id}", get(jobs::get).patch(jobs::update).delete(jobs::delete))
        .route("/jobs/{id}/run", post(jobs::run_now))
        .route("/jobs/{id}/test", post(watcher::test))
        .route("/jobs/{id}/secrets", post(jobs::set_secret))
        .route("/jobs/{id}/secrets/{name}", delete(jobs::delete_secret))
        .route("/runs/{id}", get(jobs::get_run))
        .route("/runs/{id}/output", get(jobs::run_output))
        .route("/runs/{id}/stop", post(jobs::stop_run))
        .route("/runs/{id}/retry", post(jobs::retry_run))
        .route("/runs/{id}/files", get(files_api::run_files))
        .route("/runs/{id}/approvals/{request}", post(jobs::approve))
        .route("/runs/{id}/review", get(code::review).post(code::decide))
        // Scripts
        .route("/scripts/templates", get(scripts::list_templates))
        .route("/computers/{id}/scripts", post(scripts::create))
        .route("/computers/{id}/scripts/template", post(scripts::add_template))
        // Watcher
        .route("/watcher/types", get(watcher::types).post(watcher::create_type))
        .route("/watcher/types/{id}/{version}/approve", post(watcher::approve_type))
        .route("/watcher/describe", post(watcher::describe))
        .route("/computers/{id}/watches", post(watcher::create))
        // Code
        .route("/computers/{id}/code/projects", get(code::projects).post(code::create_project))
        .route("/computers/{id}/code/tree", get(code::tree))
        .route("/computers/{id}/code/tasks", get(code::tasks).post(code::create_task))
        .route("/computers/{id}/code/devserver", get(code::dev_servers).post(code::start_dev_server))
        .route("/computers/{id}/previews", post(code::open_preview))
        // Assistant
        .route("/assistant", get(assistant::history).post(assistant::ask))
        // Admin
        .route("/admin/catalog", get(admin::catalog).post(admin::save_catalog))
        .route("/admin/accounts", get(admin::accounts))
        .route("/admin/accounts/{id}/credit", post(admin::credit))
        .route("/admin/accounts/{id}/catalog", post(admin::move_version))
        .route("/admin/measurements", get(admin::measurements))
        .route("/admin/ai/kill", post(admin::ai_kill));

    if app.cfg.dev_tools {
        api = api
            .route("/dev/outbox", get(dev::outbox))
            .route("/dev/state", get(dev::state))
            .route("/dev/clock", post(dev::advance_clock))
            .route("/dev/usage", post(dev::simulate_usage))
            .route("/dev/demo/listing", post(dev::add_listing))
            .route("/dev/demo/page", post(dev::change_page))
            .route("/dev/demo/stock", post(dev::nudge_stock));
    }

    Router::new()
        .nest("/api", api)
        .route("/demo/listings", get(demo::listings))
        .route("/demo/page", get(demo::page))
        .route("/healthz", get(|| async { "ok" }))
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(app.clone())
        .merge(app.relay.router())
}
