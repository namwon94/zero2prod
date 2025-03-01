use crate::helpers::{spawn_app, assert_is_redirect_to};

#[tokio::test]
async fn you_must_be_logged_in_to_access_the_admin_dashboard() {
    //Arrange
        let app = spawn_app().await;

        let response = app.get_admin_dashboard().await;
    
        //Assert
        assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn logout_clears_session_state() {
        //Arrange
        let app = spawn_app().await;

        //Act - Part 1 - 로그인한다.
        let login_body = serde_json::json!({
                "username": &app.test_user.username,
                "password": &app.test_user.password
        });
        let response = app.post_login(&login_body).await;
        assert_is_redirect_to(&response, "/admin/dashboard");

        //Act - Part2 - 리다이렉트를 따른다.
        let html_page = app.get_admin_dashboard_html().await;
        assert!(html_page.contains(&format!("Welcome {}", app.test_user.username)));

        //Act - Part3 - 로그아웃한다.
        let response = app.post_logout().await;
        assert_is_redirect_to(&response, "/login");

        //Act - Part4 - 리다이렉트를 따른다.
        let html_page = app.get_login_html().await;
        assert!(html_page.contains("<p><i>You have successfully logged out.</i></p>"));

        //Act - Part5 - 관리자 패널을 로딩한다.
        let response = app.get_admin_dashboard().await;
        assert_is_redirect_to(&response, "/login");
}

