use std::net::TcpListener;
//use actix_web::web::Json;
use zero2prod::startup::run;
use zero2prod::configuration::get_configuration;
//use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
//use secrecy::ExposeSecret;

/*
   'init'는 'set_logger'를 호출한다. 다른 작업은 필요하지 않다.
    RUST_LOG 환경변수가 설정되어 있지 않으면 info 및 그 이상의 레벨의 모든 로그를 출력한다.
    tracing 사용으로 env_logger 주석처리
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
*/

#[tokio::main]
async fn main() -> std::io::Result<()> {      
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    // 구성을 읽을 수 없으면 패닉에 빠진다
    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(
            configuration.database.with_db()
        );
    // 하드코딩했던 '8080'을 제거했다. 해당 값은 세팅에서 얻는다.
    let address = format!("{}:{}",configuration.application.host, configuration.application.port);
    let listener = TcpListener::bind(address)?;
    run(listener, connection_pool)?.await
}