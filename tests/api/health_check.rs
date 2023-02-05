use crate::helpers::spawn_app;


//use zero2Prod::main;

// `tokio::test` is the testing equivalent of `tokio::main`.
// It also spares you from having to specify the `#[test]` attribute.
//
// You can inspect what code gets generated using
// `cargo expand --test health_check` (<- name of the test file)

#[tokio::test]
async fn health_check_works() {
    // Arrange
    let app = spawn_app().await;
    // no .await.expect("Failed to spawn our app.");

    // We need to bring in 'reqwest'
    // to perform HTTP requests against out application.
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(&format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Failed to Execute request.");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}