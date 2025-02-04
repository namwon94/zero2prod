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
    pub fn new(base_url: String, sender: SubscriberEmail, authorization_token: Secret<String>) -> Self {
        Self { 
            http_client: Client::new(), 
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
        //요청 바디를 구조체로 인코딩 가능
        let request_body = SendEmailRequest {
            from: self.sender.as_ref().to_owned(),
            to: recipient.as_ref().to_owned(),
            subject: subject.to_owned(),
            html_body: html_content.to_owned(),
            text_body: text_content.to_owned()
        };
        //reqwest에 대한 json 기능 플래그가 활성화되면, builder는 하나의 json메서드를 노출한다. 이를 활용하면 request_body를 요청의 JSON 바디로 설정할 수 있다.
        self
            .http_client.post(&url)
            .header("X-Postmark-Server-Token", self.authorization_token.expose_secret())
            .json(&request_body)
            .send()
            .await?;
        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest {
    from: String,
    to: String,
    subject: String,
    html_body: String,
    text_body: String
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

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        //Arrange
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let email_client = EmailClient::new(mock_server.uri(), sender, Secret::new(Faker.fake()));

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();

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
            .send_email(subscriber_email, &subject, &content, &content)
            .await;

        //Assert
        //mock 기댓값은 해제 시 체크한다.
    }
}