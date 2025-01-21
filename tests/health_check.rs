/*
    tokio::test는 테스트에 있어서 tokio::main과 동등하다
    #[test] 속성을 지정하는 수고를 덜 수 있다

    cargo expand --test health_checl을 사용해서 코드가 무엇을 생성하는지 확이할 수 있다.
*/

use std::net::TcpListener;

#[tokio::test]
async fn health_check_works() {
    //Arrange(준비)
    let address = spawn_app();
    //reqwest 를 사용해서 애플리케이션에 대한 HTTP 요청을 수행한다
    let client = reqwest::Client::new();

    //Act(조작)
    let response = client
        .get(&format!("{}/health_check", &address))
        .send()
        .await
        .expect("Faules to exectue request");

    //Assert(결과 확인)
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_return_a_200_for_valid_form_data() {
    //Arrange
    let app_address = spawn_app();
    let client = reqwest::Client::new();

    //Act
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app_address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await.expect("Failed to execute request");

    //Assert
    assert_eq!(200, response.status().as_u16());
}

//테이블 주도 테스트( 파라미터화 테스트 ) -> 잘못된 입력을 다룰 때 유용함 -> 러스트 에코시스템에서는 서드 퍼티 크레이트인 rstest가 유사한 기능을 가짐
#[tokio::test]
async fn subscribe_return_a_400_when_data_is_missing() {
    //Arrange
    let app_address = spawn_app();
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the name"),
        ("email=ursual_le_guin%40gmail.com", "missing the email"),
        ("", "missing both name and emial")
    ];

    for(invaild_body, error_message) in test_cases {
        //Act
        let response = client
            .post(&format!("{}/subscriptions", &app_address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invaild_body)
            .send()
            .await.expect("Failed to execute request");
        
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


// .await를 호출하지 않으므로 비동기처리(async)가 아니여도 된다.
// 테스트를 실행하고 있으므로, 오류를 전파하지 않아도 된다.
// 만약 필요한 셋업을 수행하는 데 실해한다면, 모즌 작업을 깨뜨리면 된다.
fn spawn_app() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind random port");
    //OS가 할당한 포트 번호를 추출한다.
    let port = listener.local_addr().unwrap().port();
    let server = zero2prod::startup::run(listener).expect("Failed to bind address");
    let _ = tokio::spawn(server);
    //애플리케이션 주소를 호출자에게 반환한다.
    format!("http://127.0.0.1:{}", port)
}

/* 
use zero2prod::main 이 에러나는 이류 프로젝트를 라이브러리와 바이너리로 리팩터링을 안했기 때문 
    모든 로직은 라이브러리 크레이트에 존재, 바이ㅓ리 자체는 매우 작은 main  gkatnfmf rkwls dpsxmflvhdlsxmrk ehla
*/