use actix_web::error::InternalError;
use actix_web::HttpResponse;
use actix_web::http::header::LOCATION;
//use actix_web::http::StatusCode;
use actix_web::web;
//use actix_web::ResponseError;
use secrecy::Secret;
use sqlx::PgPool;
use crate::authentication::{validate_credentials, Credentials, AuthError};
use crate::routes::error_chain_fmt;
//20250225 추가
//use hmac::{Hmac, Mac};
//use secrecy::ExposeSecret;
//use crate::startup::HmacSecret;
//use actix_web::cookie::Cookie;
//20250226 추가
use actix_web_flash_messages::FlashMessage;
//use actix_session::Session;
use crate::session_state::TypedSession;

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>
}

#[tracing::instrument(
    skip(form, pool, session),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
//Pgpool을 주입해서 데이터베이스로부터 저장된 크리덴셜을 꺼낸다.
pub async fn login(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    //일시적으로 시크릿을 시크릿 문자열로 주입한다. -> 래퍼 타입을 주입한다. / secret: web::Data<Secret<String>> 에서 변경 -> 'HmacSecret'는 더 이상 필요하지 않는다.
    //secret: web::Data<HmacSecret>
    //Session -> TypedSession으로 변경
    session: TypedSession
) -> Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password
    }; 
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    match validate_credentials(credentials, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
            session.renew();
            session.insert_user_id(user_id)
                .map_err(|e| login_redirect(LoginError::UnexpectError(e.into())))?;
            Ok(HttpResponse::SeeOther().insert_header((LOCATION, "/admin/dashboard")).finish())
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthError::UnexpectError(_) => { LoginError::UnexpectError(e.into()) }
            };
            Err(login_redirect(e))
            /* 엔드포인트에서 쿼리 파라미터를 사용해서 오류 메시지를 전달하는 기능을 제거 
            let query_string = format!("error={}", urlencoding::Encoded::new(e.to_string()));
            let hmac_tag = {
                let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.0.expose_secret().as_bytes()).unwrap();
                mac.update(query_string.as_bytes());
                mac.finalize().into_bytes()
            };
            let response = HttpResponse::SeeOther().insert_header((LOCATION, format!("/login?{}&tag={:x}", query_string, hmac_tag))).finish();
            */
            // let response = HttpResponse::SeeOther()
            //     .insert_header((LOCATION, "/login"))
            //     //이제 쿠키가 없다.
            //     //.cookie(Cookie::new("_flash", e.to_string()))
            //     .finish();
            // Err(InternalError::from_response(e, response))
        }
    }
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

//오류 메시지와 함께 login 페이지로 리다이렉트한다.
fn login_redirect(e: LoginError) -> InternalError<LoginError> {
    FlashMessage::error(e.to_string()).send();
    let response = HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish();

    InternalError::from_response(e, response)
}