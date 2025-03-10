use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::utils::{e500, see_other, e400};
use actix_web::web::ReqData;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;
//20250305 추가
use crate::idempotency::IdempotencyKey;
//20250306 추가
use crate::idempotency::{get_saved_response,save_response};
//20250310 추가
use crate::idempotency::{try_processing, NextAction};

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    //20250305 추가 / 멱등성 키
    idempotency_key: String

}


#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(form, pool, email_client, user_id),
    fields(user_id=%*user_id)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    //사용자 세션에서 추출한 사용자 id를 주입한다.
    user_id: ReqData<UserId>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    //차용 검사기가 오류를 발생하지 않도록 폼을 제거해야 한다.
    let FormData {title, text_content, html_content, idempotency_key} = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    //20250310 추가 / 요청을 처리한 뒤 idempotency테이블에 행을 삽입 후 즉시 호출자에게 반환 하기위한 처리
    let transaction = match try_processing(&pool, &idempotency_key, *user_id).await.map_err(e400)? {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        }
    };
    //데이터베이스에 저장된 응담이 있다면 일찍 반환한다.
    if let Some(saved_response) = get_saved_response(&pool, &idempotency_key, *user_id).await.map_err(e500)? {
        return Ok(saved_response);
    }
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &title,
                        &html_content,
                        &text_content,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })
                    .map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    error.message = %error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid",
                );
            }
        }
    }
    success_message().send();
    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, *user_id, response).await.map_err(e500)?;
    Ok(response)
}

//20250310 추가
fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}

struct ConfirmedSubscriber {
    email: SubscriberEmail
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool
    //행복한 경우 'Result'와 'Vec'을 반환. 이를 통해 호출자는 '?'를 사용해서 에트워크 이슈나 일시적인 실패에 의한 오류들을 부풀릴수 있으며, 
    //컴파일러는 미묘한 오류를 처리하도록 강조. 이 기법에 관한 더 자세한 정보는 'http://sled.rs/errors.html'을 참조
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    //이 쿼리에서 나오는 데이터를 매핑할 때는 'Row'만 필요하다. 함수 자체 안에서 이 정의를 중첩하는 것은 이 커플링과 명확하게 통신하는 간단한 방법
    // struct Row {
    //     email:String
    // }
    //slqx::query_as! 에서 sqlx::query!로 변경 : 쿼리는 충분히 단순하며, 반환되는 데이터를 표현하기 위해 전용의 타입을 갖는 것으로부터 특별한 이득을 얻지 않아 변경
    let confirmed_subscribers = sqlx::query!(
        //Row,
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(error) => Err(anyhow::anyhow!(error)),
    })
    .collect();

    Ok(confirmed_subscribers)
}