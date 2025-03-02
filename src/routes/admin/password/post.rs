use actix_web::{HttpResponse, web};
use actix_web_flash_messages::FlashMessage;
use secrecy::{ExposeSecret, Secret};
//use serde::de::value;
//use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};
use crate::routes::admin::dashboard::get_username;
use crate::authentication::{validate_credentials, AuthError, Credentials};
use sqlx::PgPool;
//20250302 추가
use crate::authentication::UserId;

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>
}

pub async fn change_password(
    form: web::Form<FormData>,
    //TypedSession을 더 이상 주입하지 않는다.
    //session: TypedSession,
    pool: web::Data<PgPool>,
    user_id: web::ReqData<UserId>
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();

    //'Secret<String>'은 'Eq'를 구현하지 않으므로 그 내부의 'String'을 비교해야 한다.
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error(
            "You entered two different new passwords - the field values must match."
        )
        .send();
        return Ok(see_other("/admin/password"));
    }
    let username = get_username(*user_id, &pool).await.map_err(e500)?;
    let credentials = Credentials {
        username,
        password: form.0.current_password
    };
    if let Err(e) = validate_credentials(credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect.").send();
                Ok(see_other("/admin/password"))
            }
            AuthError::UnexpectError(_) => Err(e500(e).into())
        }
    }
    crate::authentication::change_password(*user_id, form.0.new_password, &pool).await.map_err(e500)?;
    FlashMessage::error("Your password has been changed.").send();
    Ok(see_other("/admin/password"))
}