pub mod auth_routes;
pub mod calendar;
pub mod cover;
pub mod demo;
pub mod leave;
pub mod school;
pub mod setup;
pub mod timetable;

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        // auth
        .route("/api/auth/signup", post(auth_routes::signup))
        .route("/api/auth/login", post(auth_routes::login))
        .route("/api/auth/me", get(auth_routes::me))
        // demo
        .route("/api/demo/seed", post(demo::demo_seed))
        .route("/api/demo/reset", post(demo::demo_reset))
        // school + users + dashboard
        .route("/api/school", get(school::get_school).put(school::update_school))
        .route("/api/users", get(school::list_users).post(school::create_user))
        .route("/api/users/{id}", put(school::update_user).delete(school::delete_user))
        .route("/api/dashboard", get(school::dashboard))
        // setup
        .route("/api/setup/all", get(setup::setup_all))
        .route("/api/subjects", post(setup::create_subject))
        .route("/api/subjects/{id}", put(setup::update_subject).delete(setup::delete_subject))
        .route("/api/teachers", post(setup::create_teacher))
        .route("/api/teachers/{id}", put(setup::update_teacher).delete(setup::delete_teacher))
        .route("/api/classes", post(setup::create_class))
        .route("/api/classes/{id}", put(setup::update_class).delete(setup::delete_class))
        .route("/api/rooms", post(setup::create_room))
        .route("/api/rooms/{id}", put(setup::update_room).delete(setup::delete_room))
        .route("/api/bell-schedule", put(setup::replace_bell_schedule))
        .route("/api/lessons", post(setup::create_lesson))
        .route("/api/lessons/{id}", put(setup::update_lesson).delete(setup::delete_lesson))
        // timetable
        .route("/api/timetable", get(timetable::timetable))
        .route("/api/generate", post(timetable::generate))
        .route("/api/timetable/entries", post(timetable::create_entry))
        .route(
            "/api/timetable/entries/{id}",
            put(timetable::update_entry).delete(timetable::delete_entry),
        )
        .route("/api/timetable/clear", post(timetable::clear_entries))
        .route("/api/export/timetable.csv", get(timetable::export_csv))
        // leave
        .route("/api/leave", get(leave::list_leave).post(leave::create_leave))
        .route("/api/leave/{id}/decide", post(leave::decide_leave))
        .route("/api/leave/{id}/cancel", post(leave::cancel_leave))
        .route("/api/leave/{id}/impact", get(leave::leave_impact))
        // cover
        .route("/api/cover", get(cover::cover_day))
        .route("/api/cover/assign", post(cover::assign_cover))
        .route("/api/cover/history", get(cover::cover_history))
        .route("/api/cover/{id}", delete(cover::cancel_cover))
        // calendar
        .route("/api/calendar", get(calendar::list_events).post(calendar::create_event))
        .route(
            "/api/calendar/{id}",
            put(calendar::update_event).delete(calendar::delete_event),
        )
}
