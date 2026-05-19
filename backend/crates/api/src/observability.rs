//! Phase 6: structured logging + OpenTelemetry/OTLP wiring.
//!
//! When `FEATURE_OTEL=true` and `OTLP_ENDPOINT` is set, tracing spans (HTTP
//! handlers, WS frames, AI calls) are exported to an OTLP collector. The
//! console subscriber stays attached either way so logs are still readable
//! in dev.

use anyhow::{Context, Result};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace::TracerProvider, Resource};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use crate::config::Settings;

pub fn init(cfg: &Settings) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact();

    if cfg.feature_otel {
        if let Some(endpoint) = cfg.otlp_endpoint.as_deref() {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .build()
                .context("init OTLP exporter")?;

            let provider = TracerProvider::builder()
                .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
                .with_resource(Resource::new(vec![opentelemetry::KeyValue::new(
                    "service.name",
                    cfg.otel_service_name.clone(),
                )]))
                .build();

            let tracer = provider.tracer("api");
            opentelemetry::global::set_tracer_provider(provider);
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer.boxed())
                .with(otel_layer.boxed())
                .init();
            return Ok(());
        }
        tracing::warn!("FEATURE_OTEL=true but OTLP_ENDPOINT is unset; OTLP disabled");
    }

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
    Ok(())
}

pub fn shutdown() {
    opentelemetry::global::shutdown_tracer_provider();
}
