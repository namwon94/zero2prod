use secrecy::{ExposeSecret, Secret};

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application_port: u16
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
    //구성 읽기를 초기화한다.
    let settings = config::Config::builder()
        // 'configuration.yaml 이라는 파일로부터 구성값을 추가한다.
        .add_source(
            config::File::new("configuration.yaml", config::FileFormat::Yaml)
        )
        .build()?;
    //읽은 구성값을 Settings 타입으로 변환한다.
    settings.try_deserialize::<Settings>()
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