use crate::domain::SubscriberEmail;
use reqwest::Client;
use secrecy::{ExposeSecret, Secret};

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    //우발적인 로깅을 원치 않는다.
    authorization_token: Secret<String>
}

impl EmailClient {
    pub fn new(
        base_url: String, 
        sender: SubscriberEmail, 
        authorization_token: Secret<String>,
        timeout: std::time::Duration
    ) -> Self {
        //20250205 추가 timeout설정
        let http_client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap();
        Self { 
            http_client, 
            base_url, 
            sender,
            authorization_token
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str
    ) -> Result<(), reqwest::Error> {
        //'base_url'의 타입을 'String'에서 'reqwest::Url'로 변경하면 'reqwest::Url::join'을 사용하면 더 나은 구현을 할 수 있다 (이 부분은 나의 연습)
        let url = format!("{}/email", self.base_url);
        //요청 바디를 구조체로 인코딩 가능 -> 202502025 .to_owned()를 더 이상 사용하지 않음 (클론된 문자열을 저장하기 위해 많은 신규 메모리 할당은 낭비)
        let request_body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject: subject,
            html_body: html_content,
            text_body: text_content
        };
        //reqwest에 대한 json 기능 플래그가 활성화되면, builder는 하나의 json메서드를 노출한다. 이를 활용하면 request_body를 요청의 JSON 바디로 설정할 수 있다.
        self
            .http_client.post(&url)
            .header("X-Postmark-Server-Token", self.authorization_token.expose_secret())
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
//라이프타임 파라미터는 항상 아포스트로피(')로 시작한다
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    //use wiremock::matchers::any;
    use wiremock::matchers::{header, header_exists, path, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use secrecy::Secret;
    use wiremock::Request;
    use wiremock::matchers::any;
    use claim::assert_ok;
    use claim::assert_err;

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            //bodt를 JSON 값으로 파싱한다.
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                dbg!(&body);
                //필드값을 조사하지 않고, 모든 필수 필드들이 입력되었는지 확인한다.
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            }else {
                //파싱이 실패하면, 요청을 매칭하지 않는다.
                false
            }
        }
    }

    //무작위로 이메일 제목을 생성한다.
    fn subject() -> String {
        Sentence(1..2).fake()
    }
    //무작위로 이메일 내용을 생성한다.
    fn content() -> String {
        Paragraph(1..10).fake()
    }
    //무작위로 구독자 이메일을 생성한다.
    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }
    //'EmailClient'의 테스트 인스턴스를 얻는다.
    fn email_client(base_url: String) -> EmailClient {
        EmailClient::new(
            base_url, 
            email(), 
            Secret::new(Faker.fake()), 
            std::time::Duration::from_millis(200)
        )
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        //Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        //wiremock::MockServer가 요청을 받으면 마운트된 모든 mock을 반복하면서 요청이 그 조건들에 일치하는 확인 일치하는 조건은 Mock::given을 사용해서 지정
        //header_exists 사용 시 X-Postmark-Server-Token이 서버에 대한 요청에 설정되어 있는지 확인 가능
        Mock::given(header_exists("X-Postmark-Server-Token"))
            //header를 추가해서 Content-Type이 올바른 값으로 설정되어 있는지 확인 / path를 추가해서 호출된 엔드포인트를 어서트하고 / method를 추가해서 HTTP verb를 검증
            .and(header("Content-Type", "application/json"))
            .and(path("/email"))
            .and(method("POST"))
            //커스텀 matcher를 사용
            .and(SendEmailBodyMatcher)
            //유입되는 요청이 마운트된 mock의 조건과 일치하면, wiremock::MockServer는 respond_with에 지정된 내용에 따라 응답을 반환
            .respond_with(ResponseTemplate::new(200))
            //mock에대한 기대값 ex. expect(1..)은 한 번 이상의 요청, expect(1..=3)은 한 번 이상 3번 이항의 요청
            .expect(1)
            .mount(&mock_server)
            .await;

        //ACt
        let _ = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        //Assert
        //mock 기댓값은 해제 시 체크한다.
    }

    //새로운 행복한 경로 테스트
    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        //Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        //다른 테스트에 있는 모든 매처를 복사하지 않는다. 이 테스트 목적은 밖으로 내보내는 요청에 대한 어서션을 하지 않는 것이다.
        //'send_email'에서 테스트 하기 위한 경로를 트리거하기 위한 최소한의 것만 추가한다.
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        //Act
        let outcome = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        //Assert
        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        //Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            //더 이상 200이 아니다.
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        //Act
        let outcome = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        //Assert
        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        //Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        let response = ResponseTemplate::new(200)
            .set_delay(std::time::Duration::from_secs(180));
        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        //Act
        let outcome = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        //Assert
        assert_err!(outcome);
    }
}