use sqlx::PgPool;
//anyhow의 확장 트레이트를 스코프 안으로 가져온다.
use anyhow::Context;
//20250219추가
use secrecy::Secret;
use secrecy::ExposeSecret;
//20250220 추가
//use argon2::{Argon2, PasswordHash, PasswordVerifier};
//20250221 추가
use crate::telemetry::spawn_blocking_with_tracing;
//20250301 추가 및 수정
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordVerifier, Algorithm, Params, PasswordHasher, Version};

//20250219 추가 / 10장 Authentication
pub struct Credentials {
    pub username: String,
    pub password: Secret<String>
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
pub async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool
    //PublishError : 이것은 (인증만이 아니라) 'POST /newsletters'의 실패 모드를 상세히 설명하는 특별한 오류
) -> Result<uuid::Uuid, AuthError> {
    let mut user_id = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string()
    ); 

    if let Some((stored_user_id, stored_password_hash)) = get_stored_credentials(&credentials.username, &pool).await? {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    }

    spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash,credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")??;
     
    //Ok(user_id)
    //저장소에서 크리덴셜을 찾으면 'Some'으로만 설정된다. 따라서 기본 비밀번호가 제공된 비밀번호와 매칭하더라도 존재하지 않는 사용자는 인증하지 않는다. (이 시나리오에 대한 단위 테스트를 쉽게 추가할 수 있다.)
    user_id.ok_or_else(|| anyhow::anyhow!("Unknown username.")).map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_cadidate: Secret<String>
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(
        expected_password_hash.expose_secret()
    )
    .context("Failed to parse hash in PHC string format")?;

    Argon2::default()
        .verify_password(password_cadidate.expose_secret().as_bytes(), &expected_password_hash)
        .context("Invalid password")
        .map_err(AuthError::InvalidCredentials)
}

//db에 질의하는 로직을 해당 함수의 해당 span에서 추출
#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool
) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
    let row: Option<_> = sqlx::query!(
        r#"
        SELECT user_id, password_hash
        FROM users
        WHERE username = $1
        "#,
        username
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve stored credentials.")?
    .map(|row| (row.user_id, Secret::new(row.password_hash)));

    Ok(row)
}
//20250301 추가 비밀번호 변경
#[tracing::instrument(name="Change password", skip(password, pool))]
pub async fn change_password(
    user_id: uuid::Uuid,
    password: Secret<String>,
    pool: &PgPool
) ->Result<(), anyhow::Error> {
    let password_hash = spawn_blocking_with_tracing(move || compute_password_hash(password))
        .await?.context("Failed to hash password")?;
    sqlx::query!(
        r#"
        UPDATE users
        SET password_hash = $1
        WHERE user_id = $2
        "#,
        password_hash.expose_secret(),
        user_id
    )
    .execute(pool)
    .await
    .context("Failed to change user's password in the database.")?;
    Ok(())
}

fn compute_password_hash(
    password: Secret<String>
) -> Result<Secret<String>, anyhow::Error> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None).unwrap()
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)?.to_string();
    Ok(Secret::new(password_hash))
}

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials.")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectError(#[from] anyhow::Error)
}