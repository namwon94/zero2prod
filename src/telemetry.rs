use tracing::subscriber::set_global_default;
use tracing::Subscriber;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
use tracing_subscriber::fmt::MakeWriter;
use tracing_log::LogTracer;

//여러 레이어들을 하나의 tracing의 subscriber로 구성한다.
//'impl Subscriber'를 반환 타입으로 사용해서 반환된 subscriber의 실제 타입에 관한 설명을 피한다(매우 복잡함)
//반환된 subscriber를 'init_subscriber'로 나중에 전달하기 위해, 명시적으로 'Send'이고 'Sync'임을 알려야한다.
pub fn get_subscriber<Sink>(
    name: String, 
    env_filter: String,
    sink: Sink
) -> impl Subscriber + Send + Sync 
    where
        //이 이상한 구문은 higher-ranked trait bound(HRTB)이다. 기본적으로 Sink가 모든 라이프타임 파라미터 'a'에 대해 'MakeWriter' 트레이트를 구현한다는 것을 의미
        //자세한 내용은 https://doc.rist-lang.org/nomicon/hrtb/html를 참조
        Sink: for<'a>MakeWriter<'a> + Send + Sync + 'static
{
    //RUST_LOG 환경변수가 설정되어 있지 않으면 info 레벨 및 그 이상의 모든 span을 출력한다.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(env_filter));

        let formatting_layer = BunyanFormattingLayer::new(
            name,
            //포맷이 적용된 span들을 stdout으로 출력한다.
            //std::io::stdout
            sink
        );
        
        //'with' 메서드는 'SubscriberExt'에서 제공한다. 'SubscriberExt'는 'Subscriber'의 확장 트레이트이며, 'tracing_subscriber'에 의해 노출된다.
        Registry::default()
            .with(env_filter)
            .with(JsonStorageLayer)
            .with(formatting_layer)
}

//subscriber를 글로벌 기본값으로 등록해서 span 데이터를 처리한다.(한 차례만 호출되어야 한다.)
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    //모든 'log'의 이벤트를 구독자에게 리다이렉트한다.
    LogTracer::init().expect("Failed to set logger");

    //애플리케이션에서 'set_global_default'를 사용해서 span을 처리하기 위해 어떤 subscriber를 사용해야 하는지 지정할 수 있다.
    set_global_default(subscriber).expect("Failed to set subscriber");
}