use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

/*
   'init'는 'set_logger'를 호출한다. 다른 작업은 필요하지 않다.
    RUST_LOG 환경변수가 설정되어 있지 않으면 info 및 그 이상의 레벨의 모든 로그를 출력한다.
    tracing 사용으로 env_logger 주석처리
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
*/

#[tokio::main]
async fn main() -> anyhow::Result<()> {      
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    // 구성을 읽을 수 없으면 패닉에 빠진다
    let configuration = get_configuration().expect("Failed to read configuration.");
    let application = Application::build(configuration).await?;
    application.run_until_stopped().await?;
    Ok(())
}