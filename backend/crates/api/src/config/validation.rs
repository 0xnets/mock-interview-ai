//! Configuration validation.
//!
//! Designated home for any cross-field or value-range checks on a loaded
//! [`Settings`]. It currently performs no rejection: malformed values such as
//! a bad `LISTEN_ADDR` still surface at socket-bind time exactly as before,
//! rather than at config-load time. Add fail-fast checks here if startup
//! validation is wanted later.

use anyhow::Result;

use super::Settings;

pub(super) fn validate(_settings: &Settings) -> Result<()> {
    Ok(())
}
