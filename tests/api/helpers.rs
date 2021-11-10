use once_cell::sync::Lazy;
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use wiremock::MockServer;
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::startup::{get_connection_pool, Application};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

pub struct TestApp {
    pub address: String,
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

    let address = format!("http://127.0.0.1:{}", application.port());

    // Run the application
    tokio::spawn(application.run_until_stopped());

    TestApp {
        db_pool: get_connection_pool(&configuration.database),
        address,
        email_server,
    }
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
