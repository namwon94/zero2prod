use secrecy::{ExposeSecret, Secret};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use sqlx::ConnectOptions;
use crate::domain::SubscriberEmail;

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    //20250204 새 필드 추가
    pub email_client: EmailClientSettins
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    //커넥션의 암호화 요청 여부를 결정한다.
    pub require_ssl: bool
}

//20250204 추가
#[derive(serde::Deserialize)]
pub struct EmailClientSettins {
    pub base_url: String,
    pub sender_email: String,
    pub authorization_token: Secret<String>
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir()
        .expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("configuration");
    //구성 읽기를 초기화한다.
    //실행환경을 식별한다. 지정되지 않았다면 'local'로 기본 설정한다.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");
    let environment_filename = format!("{}.yaml", environment.as_str());
    let settings = config::Config::builder()
        // 'configuration.yaml 이라는 파일로부터 구성값을 추가한다.
        .add_source(
            config::File::from(configuration_directory.join("base.yaml"))
        )
        .add_source(
            config::File::from(configuration_directory.join(&environment_filename))
        )
        //환경 변수로부터 설정에 추가한(APP, '__' 접두사를 붙인다
        //E.g. 'APP_APPLICATION__PORT=5001 would set 'Settings.application.port'
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__")
        )
        .build()?;
    //읽은 구성값을 Settings 타입으로 변환한다.
    settings.try_deserialize::<Settings>()
}

//애플리케이션이 사용할 수 있는 런타임 환경
pub enum Environment {
    Local,
    Production
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production"
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either local or productoin.",
                other
            )),
        }
    }
}

impl DatabaseSettings {
    /*
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!(
                "postgres://{}:{}@{}:{}/{}",
                self.username, self.password.expose_secret(), self.host, self.port, self.database_name
            )
        )
    }
    
    pub fn connection_string_without_db(&self) -> Secret<String> {
        Secret::new(format!(
                "postgres://{}:{}@{}:{}",
                self.username, self.password.expose_secret(), self.host, self.port
            )
        )
    }
    */

    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        }else {
            //임호화된 커넥션을 시도한다. 실패하면 암호화하지 않는 커넥션을 사용한다.
            PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(&self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode)
    }

    pub fn with_db(&self) -> PgConnectOptions {
        self.without_db().database(&self.database_name);

        let mut options = self.without_db().database(&self.database_name);
        options.log_statements(tracing::log::LevelFilter::Trace);
        options
    }
}

//20250204 추가
impl EmailClientSettins {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }
}