use actix_session::{Session, SessionExt};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use std::future::{Ready, ready};
use uuid::Uuid;

pub struct TypedSession(Session);
/*
20250306 / 책과 다르게 flash_message를 세션에 저장을 한다. 
    -> 그 이유는 flash_message를 저장 하기 전에는 newletters - newsletter_creation_is_idempotent cargo test 시 에러가 남(에러 메시지를 찾을 수 없다고)
    -> 해당 오류는 flash_message가 리다이렉션 후에 사라지는 문제가 발생할 수 있는데 현재 내가 격고 있음 그러므로 세션에 메시지를 저장하는 로직 추가
*/
impl TypedSession {
    const USER_ID_KEY: &'static str = "user_id";
    const FLASH_MESSAGE_KEY: &'static str = "flash_message";

    pub fn renew(&self) {
        self.0.renew()
    }

    pub fn insert_user_id(&self, user_id: Uuid) -> Result<(), serde_json::Error> {
        self.0.insert(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, serde_json::Error> {
        self.0.get(Self::USER_ID_KEY)
    }

    pub fn log_out(self) {
        self.0.purge()
    }

    //flash_message 저장
    pub fn insert_flash_message(&self, msg_html: String) -> Result<(), serde_json::Error> {
        self.0.insert(Self::FLASH_MESSAGE_KEY, msg_html)
    }

    pub fn get_flash_message(&self) -> Result<Option<String>, serde_json::Error> {
        self.0.get(Self::FLASH_MESSAGE_KEY)
    }
}

impl FromRequest for TypedSession {
    //이것은 다음을 설명하는 복잡한 방법이다. 우리는 'Session'을 위한 'FromRequest' 구현에 의해 반환되는 것과 같은 오류를 반환한다.
    type Error = <Session as FromRequest>::Error;
    //러스트는 트레이트 안에서 'async' 구문을 아직 지원하지 않는다. From 요청은 반환 타입으로 'Future'를 기대하며, 추출기들은 이를 사용해서 비동기 동작을 수행한다.(예 HTTP 호출)
    //여기에서는 어떤 I/O도 수행하지 않으므로 'Future'로 변환한다, 그래서 'TypedSession'을 'Ready'로 감싸서 'Future'로 변환한다.
    //이 'Future'는 실행자가 처음으로 폴링할 때 감싼 값으로 해결된다
    type Future = Ready<Result<TypedSession, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}