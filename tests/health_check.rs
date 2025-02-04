/*
    tokio::test는 테스트에 있어서 tokio::main과 동등하다
    #[test] 속성을 지정하는 수고를 덜 수 있다
    cargo expand --test health_checl을 사용해서 코드가 무엇을 생성하는지 확이할 수 있다.
*/
use std::net::TcpListener;
use sqlx::{Connection, Execute, Executor, PgConnection, PgPool};
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use uuid::Uuid;
use once_cell::sync::Lazy;
//use secrecy::ExposeSecret;
use zero2prod::email_client::EmailClient;

#[tokio::test]
async fn health_check_works() {
    //Arrange(준비)
    let app = spawn_app().await;
    //reqwest 를 사용해서 애플리케이션에 대한 HTTP 요청을 수행한다
    let client = reqwest::Client::new();

    //Act(조작)
    let response = client
        .get(&format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Faules to exectue request");

    //Assert(결과 확인)
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_return_a_200_for_valid_form_data() {
    //Arrange
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    //Act
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request");

    
    //Assert
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name from subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

//테이블 주도 테스트( 파라미터화 테스트 ) -> 잘못된 입력을 다룰 때 유용함 -> 러스트 에코시스템에서는 서드 퍼티 크레이트인 rstest가 유사한 기능을 가짐
#[tokio::test]
async fn subscribe_return_a_400_when_data_is_missing() {
    //Arrange
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the name"),
        ("email=ursual_le_guin%40gmail.com", "missing the email"),
        ("", "missing both name and emial")
    ];

    for(invaild_body, error_message) in test_cases {
        //Act
        let response = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invaild_body)
            .send()
            .await.expect("Failed to execute request");
        
        //Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            //테스트 실패 시 출력할 커스터마이즈된 추가 오류 메시지
            "The API did not fail with 400 Bad Request when the paylod was {},",
            error_message
        );
    }
}

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
// .await를 호출하지 않으므로 비동기처리(async)가 아니여도 된다. -> 이제는 비동기 함수이다.(20250121)
// 테스트를 실행하고 있으므로, 오류를 전파하지 않아도 된다.
// 만약 필요한 셋업을 수행하는 데 실해한다면, 모즌 작업을 깨뜨리면 된다.
/* 
use zero2prod::main 이 에러나는 이류 프로젝트를 라이브러리와 바이너리로 리팩터링을 안했기 때문 
    모든 로직은 라이브러리 크레이트에 존재, 바이너리 자체는 매우 작은 main 함수를 가진 엔트리포인트가 됨.
*/
async fn spawn_app() -> TestApp {
    //'initialize'가 첫번째 호출되면 'TRACING' 안의 코드가 실행된다. 다른 모든 호출은 실행을 건너뛴다.
    Lazy::force(&TRACING);


    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind random port");
    //OS가 할당한 포트 번호를 추출한다.
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    /*
    let configuration = get_configuration().expect("Failed to read configuration.");
        -> configuration.database.connection_string() : configuration.yaml 파일 안에 지정된 database_name을 사용한다.
    */
    //let configuration = get_configuration().expect("Failed to read configuration.");
    let mut configuration = get_configuration().expect("Failed to read configuration,");
    configuration.database.database_name = Uuid::new_v4().to_string();

    let connection_pool = configure_database(&configuration.database)
        .await;

    //20250204 추가
    let sender_email = configuration.email_client.sender()
        .expect("Invalid sender email address");
    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.authorization_token
    );
    //20250204 추가

    let server = run(listener, connection_pool.clone(), email_client)
        .expect("Failed to bind address");
    let _ = tokio::spawn(server);
    TestApp {
        address,
        db_pool: connection_pool
    }
    //애플리케이션 주소를 호출자에게 반환한다.
    //format!("http://127.0.0.1:{}", port)
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
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

#[tokio::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_emtpy() {
    //Arrange
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not_an-email", "invaild email")
    ];

    for (body, description) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request");

        //Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 400 Bad Request when the payload was {}",
            description
        );
    }
}