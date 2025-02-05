use crate::helpers::spawn_app;

#[tokio::test]
async fn health_check_works() {
    //Arrange(준비)
    let app = spawn_app().await;
    //reqwest 를 사용해서 애플리케이션에 대한 HTTP 요청을 수행한다
    let client = reqwest::Client::new();

    //Act(조작)
    let response = client
        .get(&format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Faules to exectue request");

    //Assert(결과 확인)
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}