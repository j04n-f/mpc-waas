use crate::utils::request::request_user_id;
use actix_web::error::{ErrorInternalServerError, ErrorNotFound};
use actix_web::{Error, HttpRequest, HttpResponse, web};
use sea_orm::DbConn;

use crate::db::repositories::UserRepository;

pub fn configure_protected(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/{id}").get(get_user).delete(delete_user));
}

pub async fn get_user(req: HttpRequest, db: web::Data<DbConn>) -> Result<HttpResponse, Error> {
    let user_id = request_user_id(&req)?;

    let repo = UserRepository::new(db.get_ref());

    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|e| ErrorNotFound(format!("Failed to retrieve user: {}", e)))?;

    match user {
        Some(user) => Ok(HttpResponse::Ok().json(user)),
        None => Err(ErrorNotFound(format!("User with ID {} not found", user_id))),
    }
}

pub async fn delete_user(req: HttpRequest, db: web::Data<DbConn>) -> Result<HttpResponse, Error> {
    let user_id = request_user_id(&req)?;

    let repo = UserRepository::new(db.get_ref());

    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|err| ErrorInternalServerError(format!("Database error: {err}")))?;

    if user.is_none() {
        return Err(ErrorNotFound(format!("User with ID {user_id} not found")));
    }

    let res = repo
        .delete(user_id)
        .await
        .map_err(|err| ErrorInternalServerError(format!("Failed to delete user: {err}")))?;

    match res.rows_affected {
        0 => Err(ErrorInternalServerError("Failed to delete user")),
        _ => Ok(HttpResponse::NoContent().finish()),
    }
}
