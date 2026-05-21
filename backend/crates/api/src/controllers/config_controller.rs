//! `GET/PUT /v1/me/config` — account-scoped HR configuration.
//!
//! HR config lives in `accounts.hr_config` (JSONB) and is shared by every
//! browser session for the account. `GET` returns the stored config or typed
//! defaults; `PUT` validates, normalizes, and persists it. Both routes sit
//! behind `auth::require_hr`, so the principal is always hr/admin/super_admin.

use axum::{extract::State, Json};
use persistence::repo_auth;

use crate::{
    app::AppState,
    auth::Principal,
    error::{ApiError, ApiResult},
    models::config::{HrConfig, HrConfigEnvelope},
};

pub async fn get_config(
    State(state): State<AppState>,
    principal: Principal,
) -> ApiResult<Json<HrConfigEnvelope>> {
    let stored = repo_auth::load_hr_config(&state.pools.read, principal.account_id).await?;
    let envelope = match stored {
        Some(value) => {
            let config: HrConfig = serde_json::from_value(value)
                .map_err(|e| ApiError::Internal(format!("stored hr_config is invalid: {e}")))?;
            HrConfigEnvelope {
                config: config.normalized(),
                saved: true,
            }
        }
        None => HrConfigEnvelope {
            config: HrConfig::defaults(state.cfg.default_pass_threshold),
            saved: false,
        },
    };
    Ok(Json(envelope))
}

pub async fn put_config(
    State(state): State<AppState>,
    principal: Principal,
    Json(body): Json<HrConfig>,
) -> ApiResult<Json<HrConfigEnvelope>> {
    let config = body.normalized();
    config.validate().map_err(ApiError::BadRequest)?;

    let value = serde_json::to_value(&config)
        .map_err(|e| ApiError::Internal(format!("serialize hr_config: {e}")))?;
    repo_auth::save_hr_config(&state.pools.primary, principal.account_id, &value).await?;

    Ok(Json(HrConfigEnvelope {
        config,
        saved: true,
    }))
}
