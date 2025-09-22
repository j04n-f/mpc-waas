mod api;
mod auth;
mod config;
mod db;
mod middleware;
mod utils;

use actix_web::{App, HttpServer, middleware::Logger};
use alloy::providers::ProviderBuilder;
use anyhow::Result;
use futures::future::join_all;
use sea_orm::{Database, DbConn};
use sea_orm_migration::MigratorTrait;
use std::sync::Arc;
use tonic::transport::Channel;

use crate::config::app_config::AppConfig;
use crate::db::migrations::Migrator;

async fn connect_db(database_url: &str) -> Result<DbConn> {
    let db: DbConn = Database::connect(database_url)
        .await
        .expect("Error connecting to the database");

    log::info!("Running database migrations...");

    Migrator::up(&db, None).await?;

    log::info!("Database migrations completed successfully");

    Ok(db)
}

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let app_config = AppConfig::from_env()?;

    log::info!(
        "Starting server at {}:{}",
        app_config.server.host,
        app_config.server.port
    );

    let provider = Arc::new(ProviderBuilder::new().connect_http(
        format!("{}:{}", app_config.provider.host, app_config.provider.port).parse()?,
    ));

    let p1 = Channel::from_shared(app_config.participants.participant_1.host.clone())?;
    let p2 = Channel::from_shared(app_config.participants.participant_2.host.clone())?;
    let p3 = Channel::from_shared(app_config.participants.participant_3.host.clone())?;

    let (db_result, channel_result) = futures::future::join(
        connect_db(&app_config.database.url),
        join_all([p1.connect(), p2.connect(), p3.connect()]),
    )
    .await;

    let participants = channel_result
        .into_iter()
        .collect::<Result<Vec<Channel>, _>>()?;

    let db = db_result?;

    HttpServer::new(move || {
        App::new()
            .configure(|config| {
                api::configure_routes(config, db.clone(), participants.clone(), provider.clone())
            })
            .wrap(Logger::default())
    })
    .bind(format!(
        "{}:{}",
        app_config.server.host, app_config.server.port
    ))?
    .run()
    .await
    .map_err(anyhow::Error::from)
}
