use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::ErrorUnauthorized;
use actix_web::{Error, HttpMessage};
use futures::future::{Ready, ready};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::auth::validate_token;

pub struct AuthMiddleware;

impl AuthMiddleware {
    pub fn new() -> Self {
        AuthMiddleware {}
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service: Arc::new(service),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: Arc<S>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let auth_header = match req.headers().get("Authorization") {
            Some(header) => header,
            None => {
                return Box::pin(async {
                    Err(ErrorUnauthorized("Authorization header not found"))
                });
            }
        };

        let auth_str = match auth_header.to_str() {
            Ok(s) => s,
            Err(_) => {
                return Box::pin(async {
                    Err(ErrorUnauthorized("Invalid authorization header format"))
                });
            }
        };

        if !auth_str.starts_with("Bearer ") {
            return Box::pin(async {
                Err(ErrorUnauthorized("Invalid authorization header format"))
            });
        }

        let token = auth_str.trim_start_matches("Bearer ").trim().to_string();
        let service = self.service.clone();

        Box::pin(async move {
            match validate_token(&token).await {
                Ok(token_data) => {
                    req.extensions_mut().insert(token_data.claims);
                    service.call(req).await
                }
                Err(err) => {
                    log::debug!("Token validation failed: {:?}", err);
                    Err(ErrorUnauthorized(
                        "Token validation failed. Please log in again.",
                    ))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::generate_token;
    use crate::db::models::UserModel;
    use actix_web::{App, HttpResponse, http::StatusCode, test, web};
    use chrono::DateTime;

    async fn send_req_with_header(name: &str, value: &str) -> StatusCode {
        let app = test::init_service(App::new().wrap(AuthMiddleware::new()).route(
            "/protected",
            web::get().to(|| async { HttpResponse::Ok().json("success") }),
        ))
        .await;

        let mut req_builder = test::TestRequest::get().uri("/protected");

        if !name.is_empty() {
            req_builder = req_builder.insert_header((name, value));
        }

        let req = req_builder.to_request();

        return match test::try_call_service(&app, req).await {
            Ok(resp) => resp.status(),
            Err(err) => err.error_response().status(),
        };
    }

    fn jwt_token() -> String {
        let claims = crate::auth::generate_claims(&UserModel {
            id: 123,
            username: "testuser".to_string(),
            password: "hashed_password".to_string(),
            email: "test@example.com".to_string(),
            created_on: Some(DateTime::from_timestamp(1640995200, 0).unwrap()),
            updated_on: Some(DateTime::from_timestamp(1640995200, 0).unwrap()),
        });

        generate_token(&claims).unwrap()
    }

    #[actix_web::test]
    async fn test_missing_authorization_header() {
        assert_eq!(send_req_with_header("", "").await, StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_missing_bearer_prefix() {
        assert_eq!(
            send_req_with_header("Authorization", "some_token_without_bearer").await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[actix_web::test]
    async fn test_invalid_token_format() {
        assert_eq!(
            send_req_with_header("Authorization", "Bearer invalid_token").await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[actix_web::test]
    async fn test_valid_token_success() {
        assert_eq!(
            send_req_with_header("Authorization", &format!("Bearer {}", jwt_token())).await,
            StatusCode::OK
        );
    }

    #[actix_web::test]
    async fn test_bearer_token_with_extra_whitespace() {
        assert_eq!(
            send_req_with_header("Authorization", &format!("Bearer   {}   ", jwt_token())).await,
            StatusCode::OK
        );
    }

    #[actix_web::test]
    async fn test_case_sensitive_bearer() {
        assert_eq!(
            send_req_with_header("Authorization", &format!("bearer {}", jwt_token())).await,
            StatusCode::UNAUTHORIZED
        );
    }
}
