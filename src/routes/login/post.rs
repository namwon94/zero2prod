use actix_web::HttpResponse;
use actix_web::http::header::LOCATION;
use actix_web::http::StatusCode;
use actix_web::{web, ResponseError};
use secrecy::Secret;
use sqlx::PgPool;
use crate::authentication::{validate_credentials, Credentials, AuthError};
use crate::routes::error_chain_fmt;

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>
}

#[tracing::instrument(
    skip(form, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
//Pgpool을 주입해서 데이터베이스로부터 저장된 크리덴셜을 꺼낸다.
pub async fn login(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>
) -> Result<HttpResponse, LoginError> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password
    };
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));
    // match validate_credentials(credentials, &pool).await {
    //     Ok(user_id) => {
    //         tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
    //         HttpResponse::SeeOther().insert_header((LOCATION, "/")).finish();
    //     }
    //     Err(_) => {
    //         todo!()
    //     }
    // }

    let user_id = validate_credentials(credentials, &pool).await
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
            AuthError::UnexpectError(_) => LoginError::UnexpectError(e.into())
        })?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
    Ok(HttpResponse::SeeOther().insert_header((LOCATION, "/")).finish())
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectError(#[from] anyhow::Error)
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for LoginError {
    fn error_response(&self) -> HttpResponse {
        let encoded_error = urlencoding::Encoded::new(self.to_string());
        HttpResponse::build(self.status_code()).insert_header((LOCATION, format!("/login?error={}", encoded_error))).finish()
    }
    fn status_code(&self) -> StatusCode {
        StatusCode::SEE_OTHER
        // match self {
        //     LoginError::UnexpectError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        //     LoginError::AuthError(_) => StatusCode::UNAUTHORIZED
        // }
    }
}