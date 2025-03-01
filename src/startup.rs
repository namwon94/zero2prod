use actix_session::storage::RedisSessionStore;
use actix_web::dev::Server;
//use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
//use actix_web::middleware::Logger;
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;
use crate::email_client::EmailClient;
use crate::configuration::Settings;
use crate::configuration::DatabaseSettings;
use sqlx::postgres::PgPoolOptions;
//20250211 추가 -> 20250214 수정
use crate::routes::{admin_dashboard, chang_password, change_password_form, confirm, health_check, login, login_form, publish_newsletter, subscribe};
//20250224 추가
use crate::routes::home;
//20250225 추가
use secrecy::Secret;
//20250226 추가
use actix_web_flash_messages::FlashMessagesFramework;
use secrecy::ExposeSecret;
use actix_web::cookie::Key;
use actix_session::SessionMiddleware;
//20250301 추가
use crate::routes::log_out;

//새롭게 만들어진 서버와 그 포트를 갖는 새로운 타입
pub struct Application {
    port: u16,
    server: Server
}

impl Application {
    //비동기이다. 또한 std::io::Error 대신 anyhow::Error를 반환한다.
    pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        //'build' 함수를 'Application'에 대한 생성자로 변환했다
        let connection_pool = get_connection_pool(&configuration.database);
        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address");
        let timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
            timeout
        );
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(&address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener, connection_pool, email_client, configuration.application.base_url, 
            configuration.application.hmac_secret, configuration.redis_uri
        ).await?;

         //바운드된 포트를 'Application'의 필드 중 하나로 저장한다.
        Ok(Self{port, server})
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    // 이 함수는 애플리케이션이 중지되었을 때만 값을 반환한다는 것을 명확하게 나타내는 이름을 사용한다.
    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(
    configuration: &DatabaseSettings
) -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.with_db())
}

//20250211 / 래퍼 타입을 정의해서 'subscribe' 핸들러에서 URL을 꺼낸다. acitx_web에서는 콘텍스트에서 꺼낸 값은 타입 기반 'String'을 사용하면 충돌이 발생
pub struct ApplicationBaseUrl(pub String);
//20250226 수정 / 비동기로 변경
async fn run(
    listener: TcpListener, db_pool: PgPool, email_client: EmailClient, 
    base_url: String, hmac_secret: Secret<String>, redis_uri: Secret<String>
) -> Result<Server, anyhow::Error> {
    //web::Data로 pool을 감싼다. Arc 스마트 포인터로 요약된다.
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));
    //20250226 추가 / 플래시 메시지를 조작하기 위해 미들웨어로 등록해야됨
    let secret_key = Key::from(hmac_secret.expose_secret().as_bytes());
    let message_store = CookieMessageStore::builder(secret_key.clone()).build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;
    /*
        move의 의미 
            : 소유권 이전 -> move 키워드는 클로저가 캡처하는 외부 변수들의 소유권을 클로저 내부로 이전시킵니다
              스레드 안전성 -> HttpServer는 여러 스레드에서 실행될 수 있으므로, 
                move를 사용하여 클로저가 캡처하는 모든 값의 소유권을 가져가 스레드 간 안전한 데이터 공유를 보장합니다
              변수 사용 제한 -> move 이후에는 클로저 외부에서 이동된 변수들을 더 이상 사용할 수 없습니다. 이는 데이터 레이스와 같은 동시성 문제를 방지합니다
     */
    let server = HttpServer::new(move || {
        App::new()
            //플래시 메시지 조작을 위한 미들웨어 추가
            .wrap(message_framework.clone())
            //세션 관리기능 미들웨어 추가
            .wrap(SessionMiddleware::new(redis_store.clone(), secret_key.clone()))
            //'App'에 대해 'wrap'메서드를 사용해서 미들웨어들을 추가한다.
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            //POST /subscriptions 요청에 대한 라우팅 테이블의 새 엔트리 포인트
            .route("/subscriptions", web::post().to(subscribe))
            //confrim 요청에 대한 라우팅 테이블의 새 엔트리 포인트
            .route("/subscriptions/confirm", web::get().to(confirm))
            //새로운 핸들러를 등록한다.
            .route("/newsletters", web::post().to(publish_newsletter))
            //20250224 추가 -> 더미 홈 페이지 엔드 포인트
            .route("/", web::get().to(home))
            //20250224 추가 -> 로그인 폼 / get
            .route("/login", web::get().to(login_form))
            //20250224 추가 -> 로그인 폼 / post
            .route("/login", web::post().to(login))
            //20250226 추가 -> admin/dashboard 엔트리 포인트 추가
            .route("/admin/dashboard", web::get().to(admin_dashboard))
            //20250228 추가 -> admin/password 비밀번호 변경 폼 / get
            .route("/admin/password", web::get().to(change_password_form))
            //20250228 추가 -> admin/password 비밀번호 변경 폼 / post
            .route("/admin/password", web::post().to(chang_password))
            //20250301 추가 -> admin/logout 로그아웃 엔트리 포인트 추가
            .route("/admin/logout", web::post().to(log_out))
            //커넥션을 애플리케이션 상테의 일부로 등록한다.
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(web::Data::new(HmacSecret(hmac_secret.clone())))
    })
    .listen(listener)?
    .run();
    Ok(server)
}

#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);