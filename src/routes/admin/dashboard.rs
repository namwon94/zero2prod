use actix_web::{web, HttpResponse, http::header::ContentType, http::header::LOCATION};
//use actix_session::Session;
use uuid::Uuid;
use anyhow::Context;
use sqlx::PgPool;
use crate::session_state::TypedSession;
use crate::utils::e500;


pub async fn admin_dashboard(
    //Session -> TypedSession으로 변경
    session: TypedSession,
    pool: web::Data<PgPool>
) -> Result<HttpResponse, actix_web::Error> {
    // 잠시 대기하여 비동기 저장 완료 기다리기
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        //println!("session.get_user_id() : {:?}", session.get_user_id());
        get_username(user_id, &pool).await.map_err(e500)?
    }else {
        //println!("session.get_user_id() : {:?}", session.get_user_id());
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
                    <title>Admin Dashboard</title>
                </head>
                <body>
                    <p>Welcome {username}</p>
                    <p>Availavle actions:</p>
                    <ol>
                        <li><a href="/admin/password">Change password</a></li>
                        <li>
                            <form name="logoutForm" action="/admin/logout" method="post">
                                <input type="submit" value="Logout">
                            </form>
                        </li>
                    </ol>
                </body>
            </html>
            "#
        )))
}

#[tracing::instrument(name = "Get username", skip(pool))]
pub async fn get_username(
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