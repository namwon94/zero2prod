use actix_web::HttpResponse;

//더미 구현
pub async fn publish_newsletter() -> HttpResponse {
    HttpResponse::Ok().finish()
}