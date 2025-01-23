use actix_web::{web, HttpResponse};
//더 이상 PgConnection을 임포트하지 않는다.
use sqlx::PgPool;
use chrono::Utc;
//use tracing::Instrument;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String
}   

//traccing::instrument가 비동기함수에 적용될 때는 Instrument::instrument를 사용하도록 주의해야한다.
#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]

//항상 200 OK를 반환시킴
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
    //'Result'는 'Ok'와 'Err'라는 두개의 변형(variant)를 갖는다.(성공과 실패 의미)
    //'match' 구문을 사용해서 결과에 따라 무엇을 수행할지 선택한다.
    match insert_subscriber(&pool, &form).await
    {
        Ok(_) => {
            tracing::info!("New subscriber details have been saved");
            HttpResponse::Ok().finish()
        },
        Err(e) => {
            //우리가 기대한 대로 작동하지 않은 경우, println을 사용해서 오류에 관한 정보를 잡아낸다.
            //println!("Failed to execute query: {}", e);
            tracing::error!("failed to execute query: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details int the database",
    skip(form, pool)
)]

pub async fn insert_subscriber(
    pool: &PgPool,
    form: &FormData
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
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