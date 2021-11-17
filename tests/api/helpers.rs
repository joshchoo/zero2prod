use once_cell::sync::Lazy;
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use wiremock::MockServer;
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::startup::{get_connection_pool, Application};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_pool: PgPool,
    pub email_server: MockServer,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/subscriptions", self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        let (username, password) = self.test_user().await;
        reqwest::Client::new()
            .post(&format!("{}/newsletters", &self.address))
            .basic_auth(username, Some(password))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn test_user(&self) -> (String, String) {
        let row = sqlx::query!("SELECT username, password FROM users LIMIT 1")
            .fetch_one(&self.db_pool)
            .await
            .expect("Failed to find a test user.");
        (row.username, row.password)
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body)
            .expect("Failed to deserialize request body.");
        let html = self
            .find_url(body["HtmlBody"].as_str().unwrap())
            .expect("Link not found in HTML body.");
        let plain_text = self
            .find_url(body["TextBody"].as_str().unwrap())
            .expect("Link not found in text body.");

        ConfirmationLinks { html, plain_text }
    }

    fn find_url(&self, s: &str) -> Option<reqwest::Url> {
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();
        match links.len() {
            0 => None,
            _ => {
                let raw_link: String = links[0].as_str().into();
                let mut confirmation_link = reqwest::Url::parse(&raw_link)
                    .unwrap_or_else(|_| panic!("Failed to parse URL: {}", raw_link));

                // Make sure not to call non-local APIs
                let host = confirmation_link.host_str().unwrap_or_else(|| {
                    panic!("Failed to get host string from {}", confirmation_link)
                });
                assert_eq!(host, "127.0.0.1");

                // Workaround: In production, the base URL does not require a port number. However in local development,
                // the server requires the port. Otherwise, the following GET request will fail.
                confirmation_link
                    .set_port(Some(self.port))
                    .unwrap_or_else(|_| panic!("Failed to set port: {}", self.port));
                Some(confirmation_link)
            }
        }
    }
}

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

// Decouple our app from the rest of the test.
pub async fn spawn_app() -> TestApp {
    // Execute the code in TRACING at most once. This prevents failures caused by initializing tracing multiple times.
    Lazy::force(&TRACING);

    let email_server = MockServer::start().await;

    let configuration = {
        let mut config = get_configuration().expect("Failed to read configuration.");
        // Assign a unique DB name
        config.database.database_name = Uuid::new_v4().to_string();
        // Setting the port to zero ensures we choose a random available port for each test
        config.application.port = 0;
        // Use mock server for email API
        config.email_client.base_url = email_server.uri();
        config
    };

    // Create and migrate the database
    configure_database(&configuration.database).await;

    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application.");

    let application_port = application.port();
    let address = format!("http://127.0.0.1:{}", application.port());

    // Run the application
    tokio::spawn(application.run_until_stopped());

    let test_app = TestApp {
        address,
        port: application_port,
        db_pool: get_connection_pool(&configuration.database),
        email_server,
    };

    add_test_user(&test_app.db_pool).await;

    test_app
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // The database doesn't exist yet. Hence create connection without DB name.
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres.");

    connection
        // Quotation marks neeed around {} because database name contains dashes (uuid v4).
        .execute((format!(r#"CREATE DATABASE "{}";"#, config.database_name)).as_str())
        .await
        .expect("Failed to create database.");

    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to run DB migrations.");

    connection_pool
}

async fn add_test_user(pool: &PgPool) {
    sqlx::query!(
        "INSERT INTO users (user_id, username, password) VALUES ($1, $2, $3)",
        Uuid::new_v4(),
        Uuid::new_v4().to_string(),
        Uuid::new_v4().to_string(),
    )
    .execute(pool)
    .await
    .expect("Failed to create a test user.");
}
