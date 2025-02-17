use actix_web::HttpResponse;
use actix_web::web;
use actix_web::ResponseError;
use sqlx::PgPool;
use actix_web::http::StatusCode;
use crate::email_client::EmailClient;
//anyhow의 확장 트레이트를 스코프 안으로 가져온다.
use anyhow::Context;
use crate::domain::SubscriberEmail;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String
}

//'body'에 '_' 프리픽스를 붙여서 사용되지 않은 인자에 대한 컴파이러 warning을 줄인다.
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>
) -> Result<HttpResponse, PublishError> {
    let subscribers = get_confirmed_subscribers(&pool).await?;
    for subscriber in subscribers {
        //컴파일러는 행복한 경우와 행복하지 않은 경우, 모두를 다루도록 강제한다.
        match subscriber {
            Ok(ref _subcriber) => {
                let subscriber = subscriber.unwrap();
                email_client
                    .send_email(
                        &subscriber.email, 
                        &body.title, 
                        &body.content.html, 
                        &body.content.text
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", &subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                    //이 오류 체인은 로그 레코드에 구조화된 필드로 기록한다.
                    error.cause_chain = ?error,
                    //'\'를 사용해서 긴 문자열 리터럴을 두 개의 행으로 자르고 '\n' 문자를 생성하지 않는다.
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid"
                );
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
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
    let rows = sqlx::query!(
        //Row,
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#
    )
    .fetch_all(pool)
    .await?;
    //도메인 타입으로 매핑한다.
    let confirmed_subscribers = rows.into_iter()
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber {email}),
            Err(error) => Err(anyhow::anyhow!(error))
        })
        .collect();
    Ok(confirmed_subscribers)
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    UnexpectError(#[from] anyhow::Error)
}

//같은 로직을 사용해서 'Debug'에 대한 모든 오류 체인을 얻는다.
impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::UnexpectError(_) => StatusCode::INTERNAL_SERVER_ERROR
        }
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