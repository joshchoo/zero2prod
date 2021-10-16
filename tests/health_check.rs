// Decouple our app from the rest of the test.
fn spawn_app() {
    let server = zero2prod::run().expect("Failed to bind address");
    // tokio::spawn will await Futures that it receives.
    // tokio::spawn drops the task when the tokio runtime shuts down, so we don't
    // need to worry about our Server persisting after the tests finish.
    tokio::spawn(server);
}

// These tests are not coupled to our app, besides the spawn_app call.
#[actix_rt::test]
async fn health_check_works() {
    spawn_app();
    let client = reqwest::Client::new();
    let response = client
        .get("http://127.0.0.1:8000/health_check")
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
