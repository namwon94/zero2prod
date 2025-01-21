use actix_web::{web, HttpResponse};
//더 이상 PgConnection을 임포트하지 않는다.
use sqlx::PgPool;
use chrono::Utc;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String
}   

//항상 200 OK를 반환시킴
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
    //'Result'는 'Ok'와 'Err'라는 두개의 변형(variant)를 갖는다.(성공과 실패 의미)
    //'match' 구문을 사용해서 결과에 따라 무엇을 수행할지 선택한다.
    match sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    //'get_ref'를 사용해서 'web::Data'로 감싸진 'PgConnection'에 대한 불변 참조(immutable reference)를 얻는다.
    // -> 해당 풀을 드롭-인 대체로 이용한다.
    .execute(pool.get_ref())
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            println!("Failed to execute query: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}