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
//20250206 추가 mock서버를 실행해서 Postmark의 API를 대신하게 하고 밖으로 전송되는 요청을 가로채야 됨.
use wiremock::MockServer;
//20250220 추가 비밀번호 저장 시 암호화 해시 작업으로 추가 sha3에서 argon2로 변경
//use sha3::Digest;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher, Algorithm, Params, Version};

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

//이메일 API에 대한 요청에 포함된 확인 링크 
//-> 외부로 나가는 이메일 요청으로부터 두 개의 확인링크를 추출하는 로직은 두 개의 테스트에서 중복되어짐 
//-> 그래서 기능의 나머지 부분들을 구체화하면서 이 로직에 의존하는 것들을 더 추가
pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url
}

pub struct TestApp {
    //20250211 추가
    pub port: u16,
    pub address: String,
    pub db_pool: PgPool,
    //20250206 추가
    pub email_server: MockServer,
    //20250220 추가
    pub test_user: TestUser
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
    //이메일 API에 대한 요청에 포함된 확인 링크르르 추출한다.
    pub fn get_confirmation_links(
        &self,
        email_request: &wiremock::Request
    ) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(
            &email_request.body
        ).unwrap();

        //요청 필드의 하나로부터 링크를 추출한다.
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            //웹에 대해 무작위 API를 호출하지 않는 것을 확인한다.
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(&body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(&body["TextBody"].as_str().unwrap());
        ConfirmationLinks {
            html,
            plain_text
        }
    }
    //20250217 추가 -> 20250219 수정 (인증 관련된 내용 )
    pub async fn post_newsletters(
        &self,
        body: serde_json::Value
    ) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/newsletters", &self.address))
            //무작위 크리덴셜 'reqwest'가 인코딩/포매팅 업무를 처리한다. -> 이제 무작위로 생성 안함 (test_user 매서드 생성)
            .basic_auth(&self.test_user.username, Some(&self.test_user.password))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
    //20250219 추가 -> 20250220 삭제 TestUser 구조체 생성
    /* 
    pub async fn test_user(&self) -> (String, String) {
        let row = sqlx::query!("SELECT username, password_hash FROM users LIMIT 1")
            .fetch_one(&self.db_pool)
            .await
            .expect("Failed to create test users.");
        (row.username, row.password_hash)
    }
    */
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

    //20250206 - mock 서버를 구동해서 Postmark의 API를 대신한다.
    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        //테스트 케이스마다 다른 데이터베이스를 사용한다.
        c.database.database_name = Uuid::new_v4().to_string();
        //무작위 OS 포트를 사용한다.
        c.application.port = 0;
        //202502026 - mock 서버를 이메일 API로서 사용한다.
        c.email_client.base_url = email_server.uri();
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
    let application_port = application.port();
    //포트를 얻은 뒤 애플리케이션을 시작한다.
    //let address = format!("http://127.0.0.1:{}", application.port());
    let _ = tokio::spawn(application.run_until_stopped());

    //20250219 수정 / 10장 인증 / 아직 편집자들을 위한 가입 플로가 구현이 안되어 있으면 완전한 블랙 박스 접근 방식을 사용 -> 기존 내용 백업 텍스트 참조
    let test_app = TestApp {
        //20250211 수정
        address: format!("http://localhost:{}", application_port),
        //20250211 추가
        port: application_port,
        db_pool: get_connection_pool(&configuration.database),
        email_server,
        test_user: TestUser::generate()
    };
    test_app.test_user.store(&test_app.db_pool).await;
    test_app
}

//20250219 추가 -> 20250220 삭제 TestUser 구조체 생성
/* 
async fn add_test_user(pool: &PgPool) {
    sqlx::query!(
        "INSERT INTO users ( user_id, username, password_hash)
        VALUES ($1, $2, $3)",
        Uuid::new_v4(),
        Uuid::new_v4().to_string(),
        Uuid::new_v4().to_string()
    )
    .execute(pool)
    .await
    .expect("Failed to create test users.");
}
*/
//20250220 무작위 비밀번호 저장을 위한 구조체 생성
pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string()
        }
    }

    async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        //정확한 Argon2 파라미터에 관해서는 신경쓰지 않는다. 이들은 테스팅 목적
        //20250221 수정 / 기본 비밀번호의 파라미터를 매칭한다.
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap()
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();
    
        sqlx::query!(
            "INSERT INTO users ( user_id, username, password_hash)
            VALUES ($1, $2, $3)",
            self.user_id,
            self.username,
            password_hash
        )
        .execute(pool)
        .await
        .expect("Faeild to store test user.");
    }
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