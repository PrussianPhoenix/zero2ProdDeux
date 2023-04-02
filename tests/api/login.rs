use crate::helpers::spawn_app;
use crate::helpers::assert_is_redirect_to;
// add crates to set up cookies
use reqwest::header::HeaderValue;
use std::collections::HashSet;

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    // arrange
    let app = spawn_app().await;

    // act try to login
    let login_body = serde_json::json!(
      {
          "username":"random-username",
          "password":"random-password"
      }
    );

    let response = app.post_login(&login_body).await;

    //assert
    assert_eq!(response.status().as_u16(), 303);
    assert_is_redirect_to(&response, "/login");
/*
    let cookies: HashSet<_> = response
        .headers()
        .get_all("Ser-Cookie")
        .into_iter()
        .collect();
    assert!(cookies.contains(&HeaderValue::from_str("_flash=Authentication failed").unwrap())
    );
    */
    //let flash_cookie = response.cookies().find(|c| c.name() == "_flash").unwrap();
    //assert_eq!(flash_cookie.value(), "Authentication failed");


    // act 2 follow the redirect
    let html_page = app.get_login_html().await;
    assert!(html_page.contains(r#"<p><i>Authentication failed</i></p>"#));

    //act 3 - reload the login page
    let html_page = app.get_login_html().await;

    assert!(!html_page.contains(r#"Authentication failed"#));
}

#[tokio::test]
async fn redirect_to_admin_dashboard_after_login_success(){
    // arrange
    let app = spawn_app().await;

    // act -part 1 - login
    let login_body = serde_json::json!({
       "username": &app.test_user.username,
        "password": &app.test_user.password
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    // act - part 2 - follow the redirect
    let html_page = app.get_admin_dashboard_html().await;
    assert!(html_page.contains(&format!("Welcome {}", &app.test_user.username)));
}
