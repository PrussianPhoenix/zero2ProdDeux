use crate::helpers::{assert_is_redirect_to, ConfirmationLinks, spawn_app, TestApp};
use wiremock::matchers::{any,method,path};
use wiremock::{Mock, ResponseTemplate};
use uuid::Uuid;

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    // Arrange
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        // we assert the no request is fired at postmark!
        .expect(0)
        .mount(&app.email_server)
        .await;

    //act

    //a sketch of the newsletter payload structure.
    // we might change it later on.
    let newsletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
    });

    let response = app.post_publish_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");


    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains(r#"<p><i>The newsletter issue has been published!</i></p>"#));
    //Mock verifies on drop that we haven't sent the newsletter email
}

// use the public api of the applicaiton under test to create
// an unconfirmed subscriber.
async fn create_unconfirmed_subscriber(app:&TestApp) -> ConfirmationLinks {
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

    // We now inspect the requests received by the mock postmark server
    // to retrieve the confirmation link and return it
    let email_request = &app.email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(&email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    // we can then reuse the same helper and just add
    // an extra step to actually call the confirmation link!
    let confirmation_link = create_unconfirmed_subscriber(app).await.html;
    reqwest::get(confirmation_link)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
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

    //act 1 submit the newsletter form
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",

    });
    let response = app.post_publish_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // act 2 follow the redirect
    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains(r#"<p><i>The newsletter issue has been published!</i></p>"#));
    //Mock verifies on Drop that we have sent the newsletter email
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
    });
    let response = app.post_publish_newsletter(&newsletter_request_body).await;

    // Assert
    assert_is_redirect_to(&response, "/login");
}