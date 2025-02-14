use actix_web::{web, HttpResponse};
//더 이상 PgConnection을 임포트하지 않는다.
use sqlx::{PgPool, Postgres, Transaction};
use chrono::Utc;
//use tracing::Subscriber;
//use tracing::Instrument;
use uuid::Uuid;
use crate::domain::{NewSubscriber, SubscriberName, SubscriberEmail};
//20250206 추가
use crate::email_client::EmailClient;
//20250211 추가
use crate::startup::ApplicationBaseUrl;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
//20250212 추가
use actix_web::ResponseError;
//20250213
use actix_web::http::StatusCode;
use anyhow::Context;

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
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;
    let mut transaction = pool.begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .context("Failed to insert new subscriber in the database")?;
    let subscription_token = generate_subscription_token();
    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .context("Failed to store the confirmation token for a new subscriber")?;
    transaction.commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber")?;
    send_confirmation_email(&email_client, new_subscriber, &base_url.0, &subscription_token)
        .await
        .context("Failed to send a confirmation email")?;

    Ok(HttpResponse::Ok().finish())
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
    skip(new_subscriber, transaction)
)]

pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
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
    .execute(transaction)
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
    skip(subscription_token, transaction)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens ( subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        //기본오류 감싸기 /20250212 수정
        //StoreTokenError(e)
        tracing::error!("Failed to execute query: {:?}]", e);
        e
    })?;
    Ok(())
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    //'Display' 와 'source'의 구현 모두를 'UnexpectError'로 감싼 타입에 투명하게 위임한다.
    #[error(transparent)]
    UnexpectError(#[from] anyhow::Error)
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectError(_) => StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

//새로운 에러 타입, sqlx::Error를 감싼다. 'Debug'를 활용한다. 쉽고 힘들지 않음
pub struct StoreTokenError(sqlx::Error);

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while \
            trying to store a subscription token."
        )
    }
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
