use actix_web::{web, HttpResponse};
//더 이상 PgConnection을 임포트하지 않는다.
use sqlx::PgPool;
use chrono::Utc;
//use tracing::Instrument;
use uuid::Uuid;
use crate::domain::{NewSubscriber, SubscriberName, SubscriberEmail};
//20250206 추가
use crate::email_client::EmailClient;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String
}   

//와이어 포맷(HTML 폼에서 수집한 url-decoded 데이터)을 도메인 모델(NewSubscriber)로 변환한다.
impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self {email, name})
    }
}

//traccing::instrument가 비동기함수에 적용될 때는 Instrument::instrument를 사용하도록 주의해야한다.
#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool, email_client),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]

//유입되는 HTTP 요청에 대해 HTTP 응답을 생성한다.
pub async fn subscribe(
    form: web::Form<FormData>, 
    pool: web::Data<PgPool>,
    //20250206 추가 - 앱 콘테스트에서 이메일 클라이언트를 얻는다.
    email_client: web::Data<EmailClient>
) -> HttpResponse {
    let new_subscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    if insert_subscriber(&pool, &new_subscriber).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }
    if send_confirmation_email(&email_client, new_subscriber).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
    
    //(쓸모없는) 이메일을 신규 가입자에게 전송한다. 지금은 이메일 전송 오류는 무시한다.
}

#[tracing::instrument(
    name = "Send a confirmation eamil to a new subscriber",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber
) -> Result<(), reqwest::Error> {
    let confirmation_link = "https://my-api.com/subscriptions/confirm";
    let plain_body = format!(
        "Welcome to our newsletter!\nvisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_body = format!(
        "Welcome to our newsletter<br />\
        Click <a href=\"{}>hereM/a> to confirm your subscription.",
        confirmation_link

    );

    email_client.send_email(new_subscriber.email, "Welcome!", &html_body, &plain_body).await
}
//입력이 subscriber 이름에 대한 검증 제약 사항을 모두 만족하면 'true'를 반환한다.
//그렇지 않으면 'false'를 반환한다.
/* 
pub fn is_valid_name(s: &str) -> bool {
    //'.trim()'은 입력 's'에 대해 뒤로 계속되는 공백 문자가 없는 뷰를 반환한다.
    //'.is_empty'는 해당 뷰가 문자를 포함하고 있는지 확인한다.
    let is_empty_or_whitespace = s.trim().is_empty();
    //grapheme는 "사용자가 인지할 수 있는" 문자로서 유니코드 표준에 의해 정의된다.
    //'a'는 단일 grapheme이지만, 두 개의 문자가 조합된 것이다. (a 와 *)
    //grapheme 입력 's'안의 grapheme에 대한 이터레이터를 반환한다.
    //'true'는 우리가 확장된 grapheme 정의 셋, 즉 권장되는 정의 셋을 사용하기 원함을 의미한다.
    let is_too_long = s.graphemes(true).count() > 256;

    //입력 's'의 모든 문자들에 대해 반복하면서 forbidden 배열  안에 있는 문자 중, 어느 하나와 일치하는 문자가 있는지 확인한다.
    let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
    let contains_forbidden_characters = s
        .chars()
        .any(|g| forbidden_characters.contains(&g));

    //어떤 한 조건이라도 위반하면 'false'를 반환한다.
    !(is_empty_or_whitespace || is_too_long || contains_forbidden_characters)
}
*/

#[tracing::instrument(
    name = "Saving new subscriber details int the database",
    skip(new_subscriber, pool)
)]

pub async fn insert_subscriber(
    pool: &PgPool,
    new_subscriber: &NewSubscriber
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        Uuid::new_v4(),
        new_subscriber.email.as_ref(),
        // 'as_ref'를 사용한다.
        new_subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}]", e);
        e   
    //'?'연산자를 사용해서 함수가 실패하면, 조기에 sqlx__Error를 반환한다. (오류 핸들링은 뒤에서 자세히)
    })?;
    Ok(())
}