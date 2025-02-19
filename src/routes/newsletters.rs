use actix_web::HttpResponse;
use actix_web::web;
use actix_web::ResponseError;
use sqlx::PgPool;
use actix_web::http::{StatusCode, header};
use crate::email_client::EmailClient;
//anyhow의 확장 트레이트를 스코프 안으로 가져온다.
use anyhow::Context;
use crate::domain::SubscriberEmail;
//20250219추가
use secrecy::Secret;
use secrecy::ExposeSecret;
use actix_web::HttpRequest;
use actix_web::http::header::{HeaderMap, HeaderValue};

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

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(body, pool, email_client, request),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
//'body'에 '_' 프리픽스를 붙여서 사용되지 않은 인자에 대한 컴파이러 warning을 줄인다.
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    //20250219 추가
    request: HttpRequest
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
    //20250219 10장 인증
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;
    tracing::Span::current().record(
        "username",
        &tracing::field::display(&credentials.username)
    );
    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));


    Ok(HttpResponse::Ok().finish())
}

struct ConfirmedSubscriber {
    email: SubscriberEmail
}

//20250219 추가 / 10장 Authentication
struct Credentials {
    username: String,
    password: Secret<String>
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    //헤더값이 존재한다면 유효한 UTF8문자열이어야 한다.
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string")?;
    let base64encoded_segment = header_value
        .strip_prefix("Basic")
        .context("The authorization scheme was not 'Basic'.")?;
    let decoded_bytes = base64::decode_config(base64encoded_segment, base64::STANDARD)
        .context("Failed to base64-decode 'Basic' credentials.")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    //':' 구분자를 사용해서 두 개의 세그먼트로 나눈다.
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| {
            anyhow::anyhow!("A username must be provided in 'Basic' auth.")
        })?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| {
            anyhow::anyhow!("A password must be provided in 'Basic' auth.")
        })?
        .to_string();

    Ok(Credentials {
        username,
        password: Secret::new(password)
    })
}

async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool
) -> Result<uuid::Uuid, PublishError> {
    let user_id: Option<_> = sqlx::query!(
        r#"
        SELECT user_id
        FROM users
        WHERE username = $1 AND password = $2
        "#,
        credentials.username,
        credentials.password.expose_secret()
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")
    .map_err(PublishError::UnexpectError)?;

    user_id .map(|row| row.user_id)
        .ok_or_else(|| anyhow::anyhow!("Invalid username or password."))
        .map_err(PublishError::AuthError)
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
    UnexpectError(#[from] anyhow::Error),
    //20250219 추가 / 인증에 관련되 에러
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error)
}

//같은 로직을 사용해서 'Debug'에 대한 모든 오류 체인을 얻는다.
impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

//20250219 수정
impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            //인증 오류에 대해 401을 반환한다.
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#)
                    .unwrap();
                response
                    .headers_mut()
                    //actix_web::http:header는 여러 잘 알려진/표준 HTTP 헤더의 이름에 관한 상수 셋을 제공한다.
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
        }
    }

    //'status_code'는 기본 'error_response' 구현에 의해 호출된다.
    //맞춤형의 'error_response' 구현을 제공하므로, 'status_code' 구현을 더 이상 유지할 필요가 없다.
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