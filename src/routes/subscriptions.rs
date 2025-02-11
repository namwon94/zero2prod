use actix_web::{web, HttpResponse};
//더 이상 PgConnection을 임포트하지 않는다.
use sqlx::{pool, PgPool};
use chrono::Utc;
//use tracing::Instrument;
use uuid::Uuid;
use crate::domain::{NewSubscriber, SubscriberName, SubscriberEmail};
//20250206 추가
use crate::email_client::EmailClient;
//20250211 추가
use crate::startup::ApplicationBaseUrl;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

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
    skip(form, pool, email_client, base_url),
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
    email_client: web::Data<EmailClient>,
    //20250211 추가 - 도메인 전달 -> 도메인과 프로토콜은 애플리케이션이 실행되는 환경에 따라 다르기 때문에 새로 추가
    base_url: web::Data<ApplicationBaseUrl>
) -> HttpResponse {
    let new_subscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish()
    };
    let subscriber_id = match insert_subscriber(&pool, &new_subscriber).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return HttpResponse::InternalServerError().finish()
    };
    let subscription_token = generate_subscription_token();
    if store_token(&pool, subscriber_id, &subscription_token).await.is_err() {
        return HttpResponse::InternalServerError().finish()
    }
    //20250211 - 애플리케이션 url을 전달한다.
    if send_confirmation_email(&email_client, new_subscriber, &base_url.0, &subscription_token).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
    
    //(쓸모없는) 이메일을 신규 가입자에게 전송한다. 지금은 이메일 전송 오류는 무시한다.
}

//대소문자를 구분하는 무작위 25문자로 구성된 구독 토큰을 생성한다.
fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "Send a confirmation eamil to a new subscriber",
    skip(email_client, new_subscriber, base_url)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    //20250211 추가
    base_url: &str,
    subscription_token: &str
) -> Result<(), reqwest::Error> {
    //동적 루트와 함께 확인 링크르르 생성한다.
    let confirmation_link = format!("{}/subscriptions/confirm?subscription_token={}", base_url, subscription_token);
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

#[tracing::instrument(
    name = "Saving new subscriber details int the database",
    skip(new_subscriber, pool)
)]

pub async fn insert_subscriber(
    pool: &PgPool,
    new_subscriber: &NewSubscriber
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        //구독자 id는 반환되거나 변수에 바운드되지 않는다.
        subscriber_id,
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
    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, pool)
)]
pub async fn store_token(
    pool: &PgPool,
    subscriber_id: Uuid,
    subscription_token: &str
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens ( subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}