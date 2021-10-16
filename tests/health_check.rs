// Decouple our app from the rest of the test.
async fn spawn_app() -> std::io::Result<()> {
    zero2prod::run().await
}

// These tests are not coupled to our app, besides the spawn_app call.
#[actix_rt::test]
async fn health_check_works() {
    spawn_app().await.expect("Failed to spawn app");
    let client = reqwest::Client::new();
    let response = client
        .get("http://127.0.0.1:8000/health_check")
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
