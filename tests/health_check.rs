use std::net::TcpListener;

// Decouple our app from the rest of the test.
fn spawn_app() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port.");
    let port = listener
        .local_addr()
        .expect("Failed to get the local socket address of the listener.")
        .port();
    let server = zero2prod::run(listener).expect("Failed to bind address");
    // tokio::spawn will await Futures that it receives.
    // tokio::spawn drops the task when the tokio runtime shuts down, so we don't
    // need to worry about our Server persisting after the tests finish.
    tokio::spawn(server);
    format!("http://127.0.0.1:{}", port)
}

// These tests are not coupled to our app, besides the spawn_app call.
#[actix_rt::test]
async fn health_check_works() {
    let address = spawn_app();
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/health_check", address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
