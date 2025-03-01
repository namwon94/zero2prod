use actix_web::{HttpResponse, http::header::ContentType};
//20250225 추가
//use crate::startup::HmacSecret;
//use hmac::{Hmac, Mac};
//use secrecy::ExposeSecret;
//20250226 추가
//use actix_web::cookie::Cookie;
use actix_web_flash_messages::IncomingFlashMessages;
//use actix_web_flash_messages::Level;
use std::fmt::Write;

//가공죄디 않은 요청에 더 이상 접근하지 않아도 된다.
pub async fn login_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    let mut error_html = String::new();
    for m in flash_messages.iter() {
        writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    //20250226 수정 / 더 이상 쿠키를 제거하지 않아도 된다.
    HttpResponse::Ok().content_type(ContentType::html()).body(format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta http-equiv="content-type" content="text/html"; charset="utf-8">
                <title>Login</title>
            </head>
            <body>
                {error_html}
                <form action="/login" method="post">
                    <label>
                        Username
                        <input type="text" placeholder="Enter Username" name="username">
                    </label>

                    <label>
                        Password
                        <input type="password" placeholder="Enter Password" name="password" value="everythinghastostartsomewhere">
                    </label>

                    <button type="submit">Login</button>
                </form>
            </body>
        </html>
        "#
    ))
}