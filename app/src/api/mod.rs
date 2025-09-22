use crate::middleware::AuthMiddleware;
use actix_web::web::ServiceConfig;
use actix_web::{HttpResponse, web};
use alloy::providers::Provider;
use sea_orm::DbConn;
use std::sync::Arc;
use tonic::transport::Channel;

mod auth;
mod users;
mod wallet;

pub fn configure_routes(
    cfg: &mut ServiceConfig,
    db: DbConn,
    participants: Vec<Channel>,
    provider: Arc<dyn Provider + Send + Sync>,
) {
    let db_data = web::Data::new(db);
    let participants_data = web::Data::new(participants);
    let provider_data = web::Data::from(provider);

    cfg.app_data(db_data)
        .app_data(participants_data)
        .app_data(provider_data)
        .route("/health", web::get().to(health_check))
        .service(
            web::scope("/api")
                .service(web::scope("/auth").configure(auth::configure))
                .service(
                    web::scope("/users")
                        .wrap(AuthMiddleware::new())
                        .configure(users::configure_protected),
                )
                .service(
                    web::scope("/wallet")
                        .wrap(AuthMiddleware::new())
                        .configure(wallet::configure),
                ),
        );
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "UP",
        "message": "Service is running"
    }))
}
