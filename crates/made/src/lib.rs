//! Composition root for MADE.
//!
//! The binary entry point (`src/main.rs`) is kept tiny: it installs
//! tracing, calls [`compose`] to wire every adapter, then runs the
//! resulting [`Application`]. All wiring logic lives here so it can
//! be unit-tested without starting a server.

mod application;
pub mod compose;
mod compose_error;
mod councils;
pub mod health;
pub mod maintenance;
pub mod runtime;
pub mod seeding;
pub mod telemetry;
pub mod workers;

pub use application::Application;
pub use compose::compose;
pub use compose_error::ComposeError;
pub use health::{router as health_router, HealthState};
pub use runtime::serve;
pub use telemetry::{init_tracing, TelemetryGuard};
