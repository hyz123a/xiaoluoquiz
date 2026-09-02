use std::{error::Error, sync::Arc};

use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use xiaoluoquiz::application::{AuthStore, hash_password};
use xiaoluoquiz::server::{
    AppState, PgAuthStore, PgPaperStore, PgQuestionStore, application_router, config::Config,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("xiaoluoquiz=info")),
        )
        .init();

    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.database_url)
        .await?;
    let question_store = Arc::new(PgQuestionStore::new(pool.clone()));
    let paper_store = Arc::new(PgPaperStore::new(pool.clone()));
    let auth_store = Arc::new(PgAuthStore::new(pool));
    let bootstrap_hash = hash_password(&config.initial_password)?;
    auth_store
        .ensure_bootstrap_admin(
            &config.initial_admin_username,
            &config.initial_admin_display_name,
            &bootstrap_hash,
        )
        .await?;
    let app = application_router(
        AppState::with_stores_and_session_ttl(
            question_store.clone(),
            auth_store,
            question_store,
            paper_store,
            config.initial_password,
            config.session_ttl_seconds,
        ),
        &config.static_dir,
    );
    let address = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&address).await?;

    tracing::info!(address = %address, "xiaoluoquiz server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
