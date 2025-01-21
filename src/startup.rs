use actix_web::dev::Server;
use std::net::TcpListener;
use actix_web::{web, App, HttpServer};
use crate::routes::{
    health_check, subscribe
};

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(health_check))
            //POST /subscriptions 요청에 ㄷ한 라우팅 테이블의 새 엔트리 포인트
            .route("/subscriptions", web::post().to(subscribe))
    })
    .listen(listener)?
    .run();
    //.await
    Ok(server)
}