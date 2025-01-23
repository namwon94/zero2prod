use actix_web::dev::Server;
//use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
//use actix_web::middleware::Logger;
use crate::routes::{
    health_check, subscribe
};
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    //web::Data로 pool을 감싼다. Arc 스마트 포인터로 요약된다.
    let db_pool = web::Data::new(db_pool);
    /*
        move의 의미 
            : 소유권 이전 -> move 키워드는 클로저가 캡처하는 외부 변수들의 소유권을 클로저 내부로 이전시킵니다
              스레드 안전성 -> HttpServer는 여러 스레드에서 실행될 수 있으므로, 
                move를 사용하여 클로저가 캡처하는 모든 값의 소유권을 가져가 스레드 간 안전한 데이터 공유를 보장합니다
              변수 사용 제한 -> move 이후에는 클로저 외부에서 이동된 변수들을 더 이상 사용할 수 없습니다. 이는 데이터 레이스와 같은 동시성 문제를 방지합니다
     */
    let server = HttpServer::new(move || {
        App::new()
            //'App'에 대해 'wrap'메서드를 사용해서 미들웨어들을 추가한다.
            .wrap(TracingLogger::default())
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