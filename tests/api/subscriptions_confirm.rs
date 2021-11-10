use linkify::{LinkFinder, LinkKind};
use reqwest::Url;
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
    let body: serde_json::Value =
        serde_json::from_slice(&email_request.body).expect("Failed to deserialize request body.");

    let raw_confirmation_link =
        &find_url(body["HtmlBody"].as_str().unwrap()).expect("Link not found in HTML body.");
    let mut confirmation_link = Url::parse(raw_confirmation_link).expect("Failed to parse URL.");
    assert_eq!(
        confirmation_link
            .host_str()
            .expect("Missing host string in confirmation link."),
        "127.0.0.1"
    );

    // Workaround: In production, the base URL does not require a port number. However in local development,
    // the server requires the port. Otherwise, the following GET request will fail.
    confirmation_link
        .set_port(Some(app.port))
        .expect("Failed to set port.");

    let response = reqwest::get(confirmation_link)
        .await
        .expect("Failed to perform GET request.");

    assert_eq!(response.status().as_u16(), 200);
}

fn find_url(s: &str) -> Option<String> {
    let links: Vec<_> = LinkFinder::new()
        .links(s)
        .filter(|l| *l.kind() == LinkKind::Url)
        .collect();
    match links.len() {
        0 => None,
        _ => Some(links[0].as_str().into()),
    }
}
