use actix_web::{web, HttpResponse, http::header::ContentType, http::header::LOCATION};
//use actix_session::Session;
use uuid::Uuid;
use anyhow::Context;
use sqlx::PgPool;
use crate::session_state::TypedSession;


pub async fn admin_dashboard(
    //Session -> TypedSession으로 변경
    session: TypedSession,
    pool: web::Data<PgPool>
) -> Result<HttpResponse, actix_web::Error> {
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        get_username(user_id, &pool).await.map_err(e500)?
    }else {
        return Ok(HttpResponse::SeeOther().insert_header((LOCATION, "/login")).finish());
    };
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta http-equiv="content-type" content="text/html"; charset="utf-8">
                    <title>Login</title>
                </head>
                <body>
                    <p>Welcome {username}</p>
                </body>
            </html>
            "#
        )))
}

#[tracing::instrument(name = "Get username", skip(pool))]
async fn get_username(
    user_id: Uuid,
    pool: &PgPool
) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username
        FROM users
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve a username")?;
    Ok(row.username)
}

//로깅을 위해 오류의 근본 원인은 유지한면서 불투명한 500을 반환한다.
fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static {
        actix_web::error::ErrorInternalServerError(e)
    }