use crate::helpers::{spawn_app, TestApp, ConfirmationLinks, assert_is_redirect_to};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};
//20250221 추가
//use uuid::Uuid;
//20250310 추가
use std::time::Duration;
//20250314 추가
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
//use wiremock::MockBuilder;

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    //이제 여러 구독자들을 다루므로, 충돌을 피하기 위해 구독자들을 무작위로 만들어야 한다.
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();
    let body = serde_urlencoded::to_string(&serde_json::json!({
        "name": name,
        "email": email
    }))
    .unwrap();

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

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    app.get_confirmation_links(email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscriber(app).await.html;
    reqwest::get(confirmation_link)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    // Arrange
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    // Act - Part 1 - Submit newsletter form
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response = app.post_publish_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 2 - Follow the redirect
    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains("<p><i>The nesletter issue has been accepted - emails will go out shortly.</i></p>"));
    app.dispatch_all_pending_emails().await;
    // Mock verifies on Drop that we haven't sent the newsletter email (Mock은 드롭 시 우리가 뉴스레터 이메일을 보내지 않았음을 검증한다.)
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    // Arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act - Part 1 - Submit newsletter form
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response = app.post_publish_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 2 - Follow the redirect
    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains("<p><i>The nesletter issue has been accepted - emails will go out shortly.</i></p>"));
    app.dispatch_all_pending_emails().await;
    // Mock verifies on Drop that we have sent the newsletter email
}

#[tokio::test]
async fn you_must_be_logged_in_to_see_the_newsletter_form() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let response = app.get_publish_newsletter().await;

    // Assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn you_must_be_logged_in_to_publish_a_newsletter() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response = app.post_publish_newsletter(&newsletter_request_body).await;

    // Assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn newsletter_creation_is_idempotent(){
    //Arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    //Act - Part 1 - 뉴스레터 폼을 제출한다.
    let idempotency_key = uuid::Uuid::new_v4().to_string();
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        //헤더가 아니라 폼 데이터의 일부로서 멱등성 키를 기대한다.
        "idempotency_key": idempotency_key
    });
    let response = app.post_publish_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    //Act - Part 2 - 리다이렉트를 따른다.
    let html_page = app.get_publish_newsletter_html().await;
    assert!(
        html_page.contains("<p><i>The nesletter issue has been accepted - emails will go out shortly.</i></p>"), "First request: {}", html_page
    );
    //Act - Part 3 - 뉴스레터 폼을 다시 제출한다.
    let response = app.post_publish_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    //Act - Part 4 - 리다이렉트를 따른다.
    let html_page = app.get_publish_newsletter_html().await;
    assert!(
        html_page.contains("<p><i>The nesletter issue has been accepted - emails will go out shortly.</i></p>"), "Second request: {}", html_page
    );
    app.dispatch_all_pending_emails().await;
    //Mock은 뉴스레터 이메일을 한 번 보냈다는 드롭을 검증한다.
}

#[tokio::test]
async fn concurrent_form_submission_is_handled_gracefully() {
    //Arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        // Setting a long delay to ensure that the second request
        // arrives before the first one completes
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    //Act - 두 개의 뉴스레터 홈을 동시에 제출한다.
    let idempotency_key = uuid::Uuid::new_v4().to_string();
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": idempotency_key
    });

    let response1 = app.post_publish_newsletter(&newsletter_request_body);
    let response2 = app.post_publish_newsletter(&newsletter_request_body);
    let (response1, response2) = tokio::join!(response1, response2);

    assert_eq!(response1.status(), response2.status());
    assert_eq!(response1.text().await.unwrap(), response2.text().await.unwrap());
    app.dispatch_all_pending_emails().await;
    //Mock은 드롭 시 이메일을 한 번만 보냈음을 검증한다.
}

//Short-hand for a common mocking setup
// fn when_sending_an_email() -> MockBuilder {
//     Mock::given(path("/email")).and(method("POST"))
// }

// #[tokio::test]
// async fn transient_errors_do_not_cause_duplicate_deliveries_on_retries() {
//     //Arrange
//     let app = spawn_app().await;
//     let idempotency_key = uuid::Uuid::new_v4().to_string();
//     let newsletter_request_body = serde_json::json!({
//         "title": "Newsletter title",
//         "text_content": "Newsletter body as plain text",
//         "html_content": "<p>Newsletter body as HTML</p>",
//         "idempotency_key": idempotency_key
//     });
//     //한 명의 구독자 대신 두 명의 구독자
//     create_confirmed_subscriber(&app).await;
//     create_confirmed_subscriber(&app).await;
//     app.test_user.login(&app).await;

//     //Part 1 - 뉴스레터 제출 폼 (두 번째 구독자에 대한 이메일 전달은 실패한다.)
//     when_sending_an_email().respond_with(ResponseTemplate::new(200)).up_to_n_times(1).expect(1).mount(&app.email_server).await;
//     when_sending_an_email().respond_with(ResponseTemplate::new(500)).up_to_n_times(1).expect(1).mount(&app.email_server).await;

//     let response = app.post_publish_newsletter(&newsletter_request_body).await;
//     assert_eq!(response.status().as_u16(), 500);

//     //Part 2 - 폼 제출을 재시도한다. (이제 2명의 구독자 모두에세 이메일 전달을 성공한다.)
//     when_sending_an_email().respond_with(ResponseTemplate::new(200)).named("delivery retry").mount(&app.email_server).await;

//     let response = app.post_publish_newsletter(&newsletter_request_body).await;
//     assert_eq!(response.status().as_u16(), 303);

//     //mock은 중복된 뉴스레터를 발송하지 않았음을 검증한다.
// }