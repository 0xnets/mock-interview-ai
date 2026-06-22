//! API configuration.
//!
//! - [`Settings`] — the typed config shape (see [`types`]).
//! - [`env`] — environment-variable loading via [`Settings::from_env`].
//! - [`defaults`] — every default value as a centralized typed constant.
//! - [`validation`] — post-load validation hook.

pub mod defaults;
mod env;
mod types;
mod validation;

pub use types::Settings;
