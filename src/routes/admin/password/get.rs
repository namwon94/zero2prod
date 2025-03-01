use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;
use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};

pub async fn change_password_form(
    session: TypedSession,
    flash_messages: IncomingFlashMessages
) -> Result<HttpResponse, actix_web::Error> {
    if session.get_user_id().map_err(e500)?.is_none() {
        return Ok(see_other("/login"));
    }

    let mut msg_html = String::new();
    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <!-- This is equivalent to a HTTP header -->
                <meta http-equiv="content-type" content="text/html"; charset="utf-8">
                <title>Change Password</title>
            </head>
            <body>
                {msg_html}
                <form action="/admin/password" method="post">
                    <label>
                        Current password
                        <input type="text" placeholder="Enter current password" name="current_password">
                    </label>

                    <label>
                        New password
                        <input type="text" placeholder="Enter new Password" name="new_password">
                    </label>
                    <br>
                    <label>
                        Confirm new password
                        <input type="text" placeholder="Type the new password agin" name="new_password_check">
                    </label>
                    <br>
                    <button type="submit">Change password</button>
                </form>
                <p><a href="/admin/dashboard">&lt;- Back</a></p>
            </body>
        </html>
        "#
    )))
}