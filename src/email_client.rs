use crate::domain::SubscriberEmail;
use reqwest::Client;

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
}

#[derive(serde::Serialize)]
struct SendEmailRequest {
    from: String,
    to: String,
    subject: String,
    text_body: String,
    html_body: String,
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            sender,
        }
    }

    pub async fn send_email(
        &self,
        subscriber_email: SubscriberEmail,
        subject: &str,
        text_content: &str,
        html_content: &str,
    ) -> Result<(), String> {
        let url = format!("{}/email", self.base_url);
        let request_body = SendEmailRequest {
            from: self.sender.as_ref().into(),
            to: subscriber_email.as_ref().into(),
            subject: subject.into(),
            text_body: text_content.into(),
            html_body: html_content.into(),
        };
        match self
            .http_client
            .post(url)
            .header("X-Postmark-Server-Token", "server_token")
            // `json` method is available when the "json" feature is enabled on the `reqwest` crate
            // It automatically sets Content-Type to "application/json"
            .json(&request_body)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err("An error occurred".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::Fake;
    use wiremock::matchers::any;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        // Start a mock HTTP server
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        // Initialize an EmailClient with the mock server's address.
        let email_client = EmailClient::new(mock_server.uri(), sender);

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            // Expect one request. The expectation is verified when MockServer goes out of scope at the end of the test.
            .expect(1)
            .mount(&mock_server) // mounting activates the mock
            .await;

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        // Create a sentence with one word
        let subject: String = Sentence(1..2).fake();
        // Create a paragraph with one to nine sentences separated by newlines
        let content: String = Paragraph(1..10).fake();

        let _ = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
    }
}
