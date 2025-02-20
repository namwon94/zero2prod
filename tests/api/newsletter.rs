use crate::helpers::{spawn_app, TestApp, ConfirmationLinks};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    //Arrange
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        //Postmark에 대한 여청이 없음을 어서트한다.
        .expect(0)
        .mount(&app.email_server)
        .await;

    //Act
    //뉴스레터 페이로드의 스켈레톤 구조
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>"
        }
    });
    let response = app.post_newsletters(newsletter_request_body).await;

    //Assert
    assert_eq!(response.status().as_u16(), 200);
    //mock은 Drop, 즉 누스레터 이메일을 보내지 않았음을 검증한다.
}

//테스트 대상 애플리케이션의 퍼블릭 API를 사용해서 확인되지 않은 구독자를 생성한다.
async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;
    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();
    //mock Postmark 서버가 받은 요청을 확인해서 확인 링크르르 추출하고 그것을 반환한다.
    let email_request = &app.email_server.received_requests().await.unwrap().pop().unwrap();
    app.get_confirmation_links(&email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    //동일한 헬퍼를 재사용해서 해당 확인 링크를 실제로 호출하는 단계를 추가한다.
    let confirmation_link = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_link.html).await.unwrap().error_for_status().unwrap();
}


#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    //Arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    //Act
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>"
        }
    });
    let response = app.post_newsletters(newsletter_request_body).await;
    //Assert
    assert_eq!(response.status().as_u16(), 200);
    //Mock은 뉴스레터 이메일을 보냈다는 Drop을 검증한다.
}

#[tokio::test]
async fn newsletters_return_400_for_invalid_data() {
    //Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>"
                }
            }),
            "missing titme"
        ),
        (
            serde_json::json!({"title": "Newsletter!"}),
            "missing content"
        )
    ];

    for(invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(invalid_body).await;

        //Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn requests_missing_authorization_are_rejected() {
    //Arrange
    let app = spawn_app().await;

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.address))
        .json(&serde_json::json!({
            "title": "Newsletter title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>"
            }
        }))
        .send()
        .await.expect("Failed to execute request.");

    //Assert
    assert_eq!(401, response.status().as_u16());
    assert_eq!(r#"Basic realm="publish""#, response.headers()["WWW-Authenticate"]);
}