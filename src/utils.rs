use actix_web::HttpResponse;
use actix_web::http::header::LOCATION;

//로깅을 위해 오류의 근본 원인은 유지한면서 불투명한 500을 반환한다.
pub fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static {
        actix_web::error::ErrorInternalServerError(e)
    }

pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther().insert_header((LOCATION, location)).finish()
}