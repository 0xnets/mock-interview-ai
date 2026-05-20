use std::net::SocketAddr;
use std::sync::Arc;

use ai::anthropic::AnthropicProvider;
use anyhow::Context;
use persistence::Pools;
use tracing::info;

mod app;
mod auth;
mod config;
mod controllers;
mod error;
mod infrastructure;
mod models;
mod rate_limit;
mod realtime;
mod services;
mod shortcode;
mod workers;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let cfg = config::Settings::from_env().context("invalid API configuration")?;

    infrastructure::observability::init(&cfg).context("init observability")?;
    infrastructure::metrics::init();

    info!(db = %mask_db_url(&cfg.database_url), "connecting to database");
    let pools = Pools::connect(
        &cfg.database_url,
        cfg.database_replica_url.as_deref(),
        cfg.db_max_connections,
    )
    .await
    .context("connect postgres")?;

    persistence::migrate::run(&pools.primary)
        .await
        .context("run migrations")?;
    info!("migrations applied");

    let provider = Arc::new(
        AnthropicProvider::new(&cfg.anthropic_api_key).context("init Anthropic provider")?,
    ) as Arc<dyn ai::AiProvider>;

    let priming =
        workers::priming::PrimingWorker::new(pools.primary.clone(), provider.clone(), cfg.clone());
    let prime_notify = priming.notify_handle();
    tokio::spawn(async move {
        priming.run().await;
    });

    // ─── Phase 6: optional Redis backplane ─────────────────────────────────
    let redis = match cfg.redis_url.as_deref() {
        Some(url) => match infrastructure::redis_backplane::RedisBackplane::connect(url).await {
            Ok(r) => {
                info!("redis backplane connected");
                if cfg.feature_redis_outbox {
                    if let Err(e) = infrastructure::streams::ensure_groups(&r).await {
                        tracing::warn!(error=%e, "ensure_groups failed");
                    }
                }
                Some(Arc::new(r))
            }
            Err(e) => {
                tracing::warn!(error=%e, "REDIS_URL set but connect failed; running without backplane");
                None
            }
        },
        None => None,
    };

    let nonces = match (cfg.feature_redis_nonces, redis.as_ref()) {
        (true, Some(r)) => Arc::new(realtime::NonceStore::new_redis(r.mgr.clone())),
        _ => Arc::new(realtime::NonceStore::new_in_memory()),
    };

    let env_signer = Arc::new(
        infrastructure::signing::TranscriptSigner::from_env().context("init transcript signer")?,
    );
    if let Err(e) = persistence::repo_keys::upsert_active(
        &pools.primary,
        env_signer.key_id(),
        &env_signer.public_key_bytes(),
        Some("startup-registered"),
    )
    .await
    {
        tracing::warn!(error=%e, "key_versions upsert failed");
    }

    let mailer = Arc::new(workers::mailer::Mailer::new(cfg.clone()));

    let outbox =
        workers::outbox::OutboxWorker::new(pools.primary.clone(), cfg.clone(), mailer.clone());
    tokio::spawn(async move {
        outbox.run().await;
    });

    // ─── Phase 6: stream consumer (if enabled) ─────────────────────────────
    if cfg.feature_redis_outbox {
        if let Some(r) = redis.clone() {
            let pool = pools.primary.clone();
            let cfg_consumer = cfg.clone();
            let consumer_id = format!("api-{}", uuid::Uuid::new_v4().simple());
            let backplane = (*r).clone();
            tokio::spawn(async move {
                infrastructure::streams::outbox_consumer_loop(
                    backplane,
                    pool,
                    cfg_consumer,
                    consumer_id,
                )
                .await;
            });
        }
    }

    let jwt = if cfg.feature_jwt_auth {
        Some(Arc::new(
            auth::JwtKeys::from_env().context("init JWT keys")?,
        ))
    } else {
        None
    };

    let rate_limit_mem = Arc::new(rate_limit::InMemoryWindow::new());

    let state = app::AppState {
        pools: pools.clone(),
        cfg: cfg.clone(),
        provider,
        prime_notify,
        nonces,
        signer: env_signer,
        redis,
        jwt,
        rate_limit_mem,
    };
    let router = app::router(state);

    // ─── Phase 6: separate Prometheus listener ─────────────────────────────
    let metrics_router = infrastructure::metrics::router();
    let metrics_addr: SocketAddr = cfg
        .metrics_listen_addr
        .parse()
        .context("invalid METRICS_LISTEN_ADDR")?;
    tokio::spawn(async move {
        info!(%metrics_addr, "prometheus /metrics listening");
        match tokio::net::TcpListener::bind(metrics_addr).await {
            Ok(listener) => {
                if let Err(e) = axum::serve(listener, metrics_router).await {
                    tracing::error!(error=%e, "metrics server stopped");
                }
            }
            Err(e) => tracing::error!(error=%e, "failed to bind metrics listener"),
        }
    });

    let addr: SocketAddr = cfg.listen_addr.parse().context("invalid LISTEN_ADDR")?;
    info!(%addr, "api listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    infrastructure::observability::shutdown();
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

fn mask_db_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let (scheme, rest) = url.split_at(scheme_end + 3);
        if let Some(at) = rest.find('@') {
            return format!("{scheme}***@{}", &rest[at + 1..]);
        }
    }
    url.to_string()
}
