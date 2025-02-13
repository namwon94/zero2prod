use crate::helpers::spawn_app;
//20250206 추가
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    //Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    //20250206 추가
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    //Act
    let response = app.post_subscriptions(body.into()).await;

    //Assert
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    //Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    //Act
    app.post_subscriptions(body.into()).await;

    //Assert
    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "pending_confirmation");
}

//테이블 주도 테스트( 파라미터화 테스트 ) -> 잘못된 입력을 다룰 때 유용함 -> 러스트 에코시스템에서는 서드 퍼티 크레이트인 rstest가 유사한 기능을 가짐
#[tokio::test]
async fn subscribe_return_a_400_when_data_is_missing() {
    //Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=le%20guin", "missing the name"),
        ("email=ursual_le_guin%40gmail.com", "missing the email"),
        ("", "missing both name and emial")
    ];

    for(invaild_body, error_message) in test_cases {
        //Act
        let response = app.post_subscriptions(invaild_body.into()).await;

        //Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            //테스트 실패 시 출력할 커스터마이즈된 추가 오류 메시지
            "The API did not fail with 400 Bad Request when the paylod was {},",
            error_message
        );
    }
}

#[tokio::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_invalid() {
    //Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not_an-email", "invaild email")
    ];

    for (body, description) in test_cases {
        let response = app.post_subscriptions(body.into()).await;


        //Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 400 Bad Request when the payload was {}",
            description
        );
    }
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    //Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    //Act
    app.post_subscriptions(body.into()).await;

    //Assert
    //mock 어서트 종료
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    //Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        //여기에서는 더 이상 기댓값을 설정하지 않는다. 테스트는 앱 동작의 다른 측면에 집중한다.
        .mount(&app.email_server)
        .await;

    //Act
    app.post_subscriptions(body.into()).await;

    //Assert
    //첫번째 가로챈 요청을 얻는다
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(&email_request);

    //두 링크는 동일해야 한다.
    assert_eq!(confirmation_links.html, confirmation_links.plain_text);
}

#[tokio::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
    //Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    //데이터베이스를 무시한다.
    sqlx::query!("ALTER TABLE subscriptions DROP COLUMN email;")
        .execute(&app.db_pool)
        .await
        .unwrap();

    //Act
    let response = app.post_subscriptions(body.into()).await;
}