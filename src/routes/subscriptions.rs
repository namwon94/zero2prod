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
    //무작위로 고유 식별자를 생성하자.
    let request_id = Uuid::new_v4();
    //Spans는 logs와 같이 연관 레벨을 갖는다. 'info_span'은 info레벨의 span을 생성한다.
    let request_span = tracing::info_span!(
        "Adding a new subscriber (tracing::info_span 사용)",
        %request_id,
        subscriber_email = %form.email,
        subscriber_name = %form.name
    );
    //async 함수에서 'enter'를 사용하면 그대로 재난이 발생한다. 지금은 잠시 참아주되, 집에서는 절대 하지 말자. 퓨처측정하기 절을 참조하자 (20250122 작성)
    let _request_span_guard = request_span.enter();
    //query_span에 대해 '.enter'를 호출하지 않는다. '.instrument' 쿼리 퓨처 수명 주기 안에서 적절한 시점에 이를 관리한다.
    let query_span = tracing::info_span!(
        "Saving new subscriber deatils in the database(tacing::info_span )"
    );
    tracing::info!(
        "request_id {} - Adding '{}' '{}' as a new subscriber (tracing::info 사용)",
        request_id,
        form.email,
        form.name
    );
    tracing::info!(
        "request_id {} - Saving new subscriber details in the database",
        request_id
    );
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