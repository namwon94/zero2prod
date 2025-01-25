use secrecy::{ExposeSecret, Secret};

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    pub port: u16,
    pub host: String,
    pub database_name: String
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
}