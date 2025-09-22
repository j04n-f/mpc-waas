use crate::auth::Claims;
use actix_web::{HttpMessage, HttpRequest};

pub fn request_user_id(req: &HttpRequest) -> Result<i32, actix_web::Error> {
    let ext = req.extensions();

    let claims = &ext
        .get::<Claims>()
        .ok_or(actix_web::error::ErrorUnauthorized("User not authorized"))?;

    Ok(claims.user_id)
}
