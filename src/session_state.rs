use actix_session::{Session, SessionExt};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use std::future::{Ready, ready};
use uuid::Uuid;

pub struct TypedSession(Session);

impl TypedSession {
    const USER_ID_KEY: &'static str = "user_id";

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn insert_user_id(&self, user_id: Uuid) -> Result<(), serde_json::Error> {
        self.0.insert(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, serde_json::Error> {
        self.0.get(Self::USER_ID_KEY)
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