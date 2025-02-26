use crate::helpers::{spawn_app, assert_is_redirect_to};
//use reqwest::header::HeaderValue;
//use std::collections::HashSet;

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    //Arrange
    let app = spawn_app().await;

    //Act
    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password"
    });
    let response = app.post_login(&login_body).await;
    //let flash_cookie = response.cookies().find(|c| c.name() == "_flash").unwrap();

    // let cookies: HashSet<_> = response.headers().get_all("Set-Cookie").into_iter().collect();
    // assert!(cookies.contains(&HeaderValue::from_str("_flash-Authentication failed").unwrap()));

    //Assert
    assert_is_redirect_to(&response, "/login");
    //assert_eq!(flash_cookie.value(), "Authentication failed");

    //Act - Part 2 - 리다이렉트를 따른다.
    let html_page = app.get_login_html().await;
    assert!(html_page.contains("<p><i>Authentication failed</i></p>"));

    //Act - Part3 - 로그인 페이지를 다시 로딩한다.
    let html_page = app.get_login_html().await;
    assert!(!html_page.contains("<p><i>Authentication Failed</i></p>"));
}