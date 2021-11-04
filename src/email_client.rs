use crate::domain::SubscriberEmail;
use reqwest::Client;

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    authorization_token: String,
}

#[derive(serde::Serialize)]
// We could use #[serde(rename_all = "PascalCase")], but I'd prefer being explicit about the naming.
struct SendEmailRequest<'a> {
    #[serde(rename = "From")]
    from: &'a str,
    #[serde(rename = "To")]
    to: &'a str,
    #[serde(rename = "Subject")]
    subject: &'a str,
    #[serde(rename = "TextBody")]
    text_body: &'a str,
    #[serde(rename = "HtmlBody")]
    html_body: &'a str,
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail, authorization_token: String) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            sender,
            authorization_token,
        }
    }

    pub async fn send_email(
        &self,
        subscriber_email: SubscriberEmail,
        subject: &str,
        text_content: &str,
        html_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/email", self.base_url);
        let request_body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: subscriber_email.as_ref(),
            subject,
            text_body: text_content,
            html_body: html_content,
        };
        self.http_client
            .post(url)
            .header("X-Postmark-Server-Token", &self.authorization_token)
            // `json` method is available when the "json" feature is enabled on the `reqwest` crate
            // It automatically sets Content-Type to "application/json"
            .json(&request_body)
            .send()
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use wiremock::matchers::{header, header_exists, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            // Verify that the request body is a valid JSON and contains all the expected properties.
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            } else {
                false
            }
        }
    }

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        // Start a mock HTTP server
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        // Initialize an EmailClient with the mock server's address.
        let email_client = EmailClient::new(mock_server.uri(), sender, Faker.fake());

        Mock::given(header_exists("X-Postmark-Server-Token"))
            .and(header("Content-type", "application/json"))
            .and(method("POST"))
            .and(path("/email"))
            .and(SendEmailBodyMatcher)
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
