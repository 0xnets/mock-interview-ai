//! Application wiring: shared [`AppState`], HTTP [`router`] composition, and
//! liveness/readiness routes.

mod health;
mod router;
mod state;

pub use router::router;
pub use state::AppState;
