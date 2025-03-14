use crate::authentication::UserId;
//use crate::domain::SubscriberEmail;
//use crate::email_client::EmailClient;
use crate::utils::{e500, see_other, e400};
use actix_web::web::ReqData;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;
//20250305 추가
use crate::idempotency::IdempotencyKey;
//20250306 추가
use crate::idempotency::save_response;
//20250310 추가
use crate::idempotency::{try_processing, NextAction};
//20250314 추가
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    //20250305 추가 / 멱등성 키
    idempotency_key: String

}

//20250314 수정 / 오류처리
#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip_all,
    fields(user_id=%&*user_id)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    //사용자 세션에서 추출한 사용자 id를 주입한다.
    user_id: ReqData<UserId>,
    pool: web::Data<PgPool>,
    //email_client: web::Data<EmailClient>
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    //차용 검사기가 오류를 발생하지 않도록 폼을 제거해야 한다.
    let FormData {title, text_content, html_content, idempotency_key} = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    //20250310 추가 / 요청을 처리한 뒤 idempotency테이블에 행을 삽입 후 즉시 호출자에게 반환 하기위한 처리
    let mut transaction = match try_processing(&pool, &idempotency_key, *user_id).await.map_err(e500)? {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        }
    };
    let issue_id = insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(e500)?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;
    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, *user_id, response).await.map_err(e500)?;
    success_message().send();
    Ok(response)
}

//20250310 추가
fn success_message() -> FlashMessage {
    FlashMessage::info("The nesletter issue has been accepted - emails will go out shortly.")
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id, title, text_content, html_content, published_at
        )
        VALUES (
            $1, $2, $3, $4, now()
        )
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    )
    .execute(transaction)
    .await?;
    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id, subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id
    )
    .execute(transaction)
    .await?;
    Ok(())
}