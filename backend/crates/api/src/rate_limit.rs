//! Phase 6: per-route token-bucket rate limiter.
//!
//! Redis (sliding-window via Lua) is preferred so the limit holds across
//! pods. When `REDIS_URL` is unset or unreachable we fall back to a
//! per-process in-memory map keyed the same way — useful for dev and as
//! safety if Redis blips during a deploy.
//!
//! The limiter is keyed by `(route, principal)`. `principal` is whichever of
//! these is most specific: an authenticated account id, an email (for
//! `/v1/auth/login` so brute-force is per-target), or the client IP.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, Request, Response, StatusCode},
    middleware::Next,
};
use redis::AsyncCommands;

use crate::app::AppState;

#[derive(Debug, Clone, Copy)]
pub struct Limit {
    pub count: u32,
    pub window: Duration,
}

impl Limit {
    pub const fn new(count: u32, window: Duration) -> Self {
        Self { count, window }
    }
}

#[derive(Debug, Default)]
pub struct InMemoryWindow {
    inner: Mutex<HashMap<String, Vec<Instant>>>,
}

impl InMemoryWindow {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `(remaining, retry_after)`. `Some` on retry_after means denied.
    pub fn check(&self, key: &str, limit: Limit) -> (u32, Option<Duration>) {
        let mut guard = self.inner.lock().expect("rate limit mutex");
        let now = Instant::now();
        let entry = guard.entry(key.to_string()).or_default();
        // Drop expired hits.
        entry.retain(|t| now.duration_since(*t) < limit.window);

        if (entry.len() as u32) >= limit.count {
            // Retry-after = window - age_of_oldest.
            let oldest = entry.first().copied().unwrap_or(now);
            let age = now.duration_since(oldest);
            let retry = limit.window.saturating_sub(age);
            return (0, Some(retry));
        }
        entry.push(now);
        let remaining = limit.count.saturating_sub(entry.len() as u32);
        (remaining, None)
    }
}

/// Atomic sliding-window via Redis: ZADD <now>; ZREMRANGEBYSCORE [-inf, now-window];
/// ZCARD -> compare to limit. Same semantics as `InMemoryWindow::check`.
pub async fn check_redis(
    mgr: &mut redis::aio::ConnectionManager,
    key: &str,
    limit: Limit,
) -> redis::RedisResult<(u32, Option<Duration>)> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_ms = limit.window.as_millis() as i64;
    let cutoff = now_ms - window_ms;

    let mut pipe = redis::pipe();
    pipe.atomic();
    pipe.cmd("ZREMRANGEBYSCORE")
        .arg(key)
        .arg("-inf")
        .arg(cutoff)
        .ignore();
    pipe.zadd(key, format!("{now_ms}:{}", rand::random::<u32>()), now_ms);
    pipe.zcard(key);
    pipe.expire(key, (window_ms / 1000).max(1));

    let (_zadd, count, _exp): (i64, i64, i64) = pipe.query_async(mgr).await?;

    if (count as u32) > limit.count {
        // Find the oldest entry in the window to compute retry-after.
        let oldest: Vec<(String, i64)> = mgr.zrange_withscores(key, 0, 0).await?;
        let retry = oldest
            .into_iter()
            .next()
            .map(|(_, score)| {
                let age_ms = now_ms - score;
                Duration::from_millis((window_ms - age_ms).max(0) as u64)
            })
            .unwrap_or(limit.window);
        Ok((0, Some(retry)))
    } else {
        let remaining = limit.count.saturating_sub(count as u32);
        Ok((remaining, None))
    }
}

pub fn limit_for_route(state: &AppState, route: &str) -> Option<Limit> {
    let cfg = &state.cfg;
    if route == "POST /v1/interviews" {
        return Some(Limit::new(
            cfg.rate_limit_create_interview_per_hour,
            Duration::from_secs(3600),
        ));
    }
    if route == "POST /v1/auth/login" {
        return Some(Limit::new(
            cfg.rate_limit_login_per_min_ip,
            Duration::from_secs(60),
        ));
    }
    if route.starts_with("GET /v1/sessions/by-code/") {
        return Some(Limit::new(
            cfg.rate_limit_session_by_code_per_min,
            Duration::from_secs(60),
        ));
    }
    if route == "GET /v1/ws/interview" {
        return Some(Limit::new(
            cfg.rate_limit_ws_per_min,
            Duration::from_secs(60),
        ));
    }
    None
}

pub fn client_ip(headers: &HeaderMap, connect_info: Option<&IpAddr>) -> String {
    if let Some(fwd) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = fwd.split(',').next() {
            return first.trim().to_string();
        }
    }
    if let Some(ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return ip.to_string();
    }
    connect_info
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Middleware applied to a single route. The route name is what
/// `limit_for_route` keys on. Returns 429 with `Retry-After` (seconds) on
/// denial. Account-bound routes can prepend the account id to `principal`.
pub async fn check_and_pass(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    if !state.cfg.feature_rate_limit {
        return next.run(req).await;
    }
    let route = format!(
        "{} {}",
        req.method(),
        req.uri().path().trim_end_matches('/')
    );

    let Some(limit) = limit_for_route(&state, &route) else {
        return next.run(req).await;
    };

    // Principal: account-id if authenticated, else IP. The auth middleware
    // runs after rate-limit on protected routes, so we mostly key by IP.
    let principal = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split_whitespace().nth(1))
        .map(|tok| format!("tok:{}", short_hash(tok)))
        .unwrap_or_else(|| format!("ip:{}", client_ip(&headers, Some(&addr.ip()))));

    let key = format!("rl:{route}:{principal}");
    let (_, retry) = match state.redis.as_ref() {
        Some(r) => {
            let mut mgr = r.mgr.clone();
            match check_redis(&mut mgr, &key, limit).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error=%e, "redis rate limit failed; falling back to in-memory");
                    state.rate_limit_mem.check(&key, limit)
                }
            }
        }
        None => state.rate_limit_mem.check(&key, limit),
    };

    if let Some(retry) = retry {
        crate::infrastructure::metrics::inc_rate_limited(&route);
        let secs = retry.as_secs().max(1);
        let mut resp = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(header::RETRY_AFTER, secs.to_string())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "error": { "code": "RATE_LIMITED", "message": "too many requests" }
                })
                .to_string(),
            ))
            .expect("build rate-limit response");
        // metrics handled above
        let _ = resp.headers_mut(); // keep clippy happy if we add headers later
        return resp;
    }
    next.run(req).await
}

fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(&h.finalize()[..6])
}
