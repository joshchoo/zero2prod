use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::spawn_app;

#[actix_rt::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    let app = spawn_app().await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);
}

#[actix_rt::test]
async fn confirmations_with_invalid_token_are_rejected_with_a_401() {
    let app = spawn_app().await;

    let response = reqwest::get(&format!(
        "{}/subscriptions/confirm?subscription_token=tokenNotInDatabase",
        app.address
    ))
    .await
    .unwrap();

    assert_eq!(response.status().as_u16(), 401);
}

#[actix_rt::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .expect("Failed to query mock server for received reqeusts.")[0];

    let confirmation_links = app.get_confirmation_links(email_request);
    assert_eq!(confirmation_links.html, confirmation_links.plain_text);

    let response = reqwest::get(confirmation_links.html)
        .await
        .expect("Failed to perform GET request.");

    assert_eq!(response.status().as_u16(), 200);
}

#[actix_rt::test]
async fn opening_the_confirmation_link_confirms_a_subscriber() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // Subscribe
    app.post_subscriptions(body.into()).await;
    let email_request = &app
        .email_server
        .received_requests()
        .await
        .expect("Failed to query mock server for received reqeusts.")[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    // Open subscription confirmation link
    reqwest::get(confirmation_links.html)
        .await
        .expect("Failed to perform GET request.")
        .error_for_status()
        .expect("Request returned HTTP error status.");

    // Check that subscription is confirmed
    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.status, "confirmed");
    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}
