//! External integrations and cross-cutting system concerns.
//!
//! Groups the modules that talk to the outside world or to the runtime
//! environment — key management, Redis, signing, streams, observability,
//! metrics, and PDF rendering — as opposed to request handling or domain logic.

pub mod metrics;
pub mod observability;
pub mod pdf;
pub mod redis_backplane;
pub mod signing;
pub mod streams;
