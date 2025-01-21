use actix_web::dev::Server;
use std::net::TcpListener;
use actix_web::{web, App, HttpServer};
use crate::routes::{
    health_check, subscribe
};
use sqlx::PgPool;

pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    //web::Data로 pool을 감싼다. Arc 스마트 포인터로 요약된다.
    let db_pool = web::Data::new(db_pool);
    let server = HttpServer::new(move || {
        App::new()
            .route("/health_check", web::get().to(health_check))
            //POST /subscriptions 요청에 대한 라우팅 테이블의 새 엔트리 포인트
            .route("/subscriptions", web::post().to(subscribe))
            //커넥션을 애플리케이션 상테의 일부로 등록한다.
            .app_data(db_pool.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}