/*
    tokio::test는 테스트에 있어서 tokio::main과 동등하다
    #[test] 속성을 지정하는 수고를 덜 수 있다
    cargo expand --test health_checkc을 사용해서 코드가 무엇을 생성하는지 확이할 수 있다.
*/
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
//use std::net::TcpListener;
use uuid::Uuid;
use zero2prod::configuration::{get_configuration, DatabaseSettings};
//use zero2prod::email_client::EmailClient;
//use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use zero2prod::startup::{Application, get_connection_pool};

//'once_cell' 을 사용해서 'TRACING' 스택이 한 번만 초기화되는 것을 보장한다.
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "into".to_string();
    let subscriber_name = "test".to_string();
    //'get_subscriber'의 출력을 'TEST_LOG'의 값에 기반해서 변수에 할당할 수 없다.
    //왜냐하면 해당 sink는 'get_subscriber'에 의해 반환된 타입의 일부이고, 그들의 타입이 같지 않기 때문이다
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(
            subscriber_name, 
            default_filter_level, 
            std::io::stdout
        );
        init_subscriber(subscriber);
    }
    else {
        let subscriber = get_subscriber(
            subscriber_name,
            default_filter_level,
            std::io::sink
        );
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }
}

// .await를 호출하지 않으므로 비동기처리(async)가 아니여도 된다. -> 이제는 비동기 함수이다.(20250121)
// 테스트를 실행하고 있으므로, 오류를 전파하지 않아도 된다.
// 만약 필요한 셋업을 수행하는 데 실해한다면, 모즌 작업을 깨뜨리면 된다.
/* 
use zero2prod::main 이 에러나는 이류 프로젝트를 라이브러리와 바이너리로 리팩터링을 안했기 때문 
    모든 로직은 라이브러리 크레이트에 존재, 바이너리 자체는 매우 작은 main 함수를 가진 엔트리포인트가 됨.
*/
pub async fn spawn_app() -> TestApp {
    //'initialize'가 첫번째 호출되면 'TRACING' 안의 코드가 실행된다. 다른 모든 호출은 실행을 건너뛴다.
    Lazy::force(&TRACING);

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        //테스트 케이스마다 다른 데이터베이스를 사용한다.
        c.database.database_name = Uuid::new_v4().to_string();
        //무작위 OS 포트를 사용한다.
        c.application.port = 0;
        c
    };
    //데이터베이스를 생성하고 마이그레이션한다.
    configure_database(&configuration.database).await;

    //애플리케이션을 백그라운드 테스크로 구동한다.
    //src/configuration의 모든 구조체에 #[derive(Clone)]를 부여서 컴파일을 행복하게 만들어야 되는데 데이터베이스 커넥션 풀에만 붙임
    /* 
    let server = build(configuration.clone()).await.expect("Failed to build application");
    let _ = tokio::spawn(server);
    */
    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application");
    //포트를 얻은 뒤 애플리케이션을 시작한다.
    let address = format!("http://127.0.0.1:{}", application.port());
    let _ = tokio::spawn(application.run_until_stopped());

    TestApp {
        address,
        db_pool: get_connection_pool(&configuration.database)
    }
    //애플리케이션 주소를 호출자에게 반환한다.
    //format!("http://127.0.0.1:{}", port)
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    //데이터베이스를 생성한다.
    let mut connection = PgConnection::connect_with(
        &config.without_db())
        .await
        .expect("Failed to connect to Postgres");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");
    
    //데이터 베이스를 마이그레이션 한다.
    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Filed to connect to Postgres");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}