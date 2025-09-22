use actix_web::error::{ErrorInternalServerError, ErrorUnauthorized, ErrorUnprocessableEntity};
use actix_web::{Error, HttpResponse, web};

use sea_orm::DbConn;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::auth::hash_password;
use crate::db::models::UserActiveModel;

use crate::utils::validators::user::{validate_no_spaces, validate_password};

use sea_orm::ActiveValue::Set;

use crate::auth::{generate_claims, generate_token, verify_password};
use crate::db::repositories::UserRepository;
use crate::utils::validate::validate_req;

#[derive(Deserialize, Validate)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/login", web::post().to(login))
        .route("/signup", web::post().to(signup));
}

async fn login(db: web::Data<DbConn>, req: web::Json<LoginRequest>) -> Result<HttpResponse, Error> {
    validate_req(&req)?;

    let user_repository = UserRepository::new(db.get_ref());

    let user = match user_repository
        .find_by_username(&req.username)
        .await
        .map_err(|e| ErrorInternalServerError(format!("Database error: {}", e)))?
    {
        Some(user) => user,
        None => return Err(ErrorUnauthorized("Account not registered".to_string())),
    };

    let is_valid = verify_password(&req.password, &user.password)?;
    if !is_valid {
        return Err(ErrorUnauthorized("Invalid credentials"));
    }

    let claims = generate_claims(&user);
    let token = generate_token(&claims)?;

    Ok(HttpResponse::Ok().json(LoginResponse { token }))
}

#[derive(Deserialize, Serialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(
        min = 3,
        max = 200,
        message = "Username must be between 3 and 50 characters"
    ))]
    #[validate(custom(function = validate_no_spaces))]
    pub username: String,

    #[validate(custom(function = validate_password))]
    pub password: String,

    #[validate(email(message = "Invalid email format"))]
    pub email: String,
}

pub async fn signup(
    db: web::Data<DbConn>,
    user: web::Json<CreateUserRequest>,
) -> Result<HttpResponse, Error> {
    validate_req(&user)?;

    let repo = UserRepository::new(db.get_ref());

    if (repo
        .find_by_username(&user.username)
        .await
        .map_err(ErrorInternalServerError)?)
    .is_some()
    {
        return Err(ErrorUnprocessableEntity(format!(
            "Username {} already exists",
            user.username
        )));
    }

    if (repo
        .find_by_email(&user.email)
        .await
        .map_err(ErrorInternalServerError)?)
    .is_some()
    {
        return Err(ErrorUnprocessableEntity(format!(
            "Email {} already exists",
            user.email
        )));
    }

    let user_model = UserActiveModel {
        username: Set(user.username.clone()),
        password: Set(hash_password(&user.password)?),
        email: Set(user.email.clone()),
        ..Default::default()
    };

    let created_user = repo
        .create(user_model)
        .await
        .map_err(|e| ErrorInternalServerError(format!("Failed to create user: {}", e)))?;

    Ok(HttpResponse::Created().json(created_user))
}
