use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
//20250317 추가
use zero2prod::issue_delivery_worker::run_worker_until_stopped;
use std::fmt::{Debug, Display};
use tokio::task::JoinError;

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
    let application = Application::build(configuration.clone()).await?;
    let application_task = tokio::spawn(application.run_until_stopped());
    let worker_task = tokio::spawn(run_worker_until_stopped(configuration));

    tokio::select! {
        o = application_task => report_exit("API", o),
        o = worker_task =>  report_exit("Background worker", o)
    };
    Ok(())
}

fn report_exit(
    task_name: &str,
    outcome: Result<Result<(), impl Debug + Display>, JoinError>
) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed",
                task_name
            )
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{}' task failed to complete",
                task_name
            )
        }
    }
}