use crate::configuration::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    /// Initializes database connections, email client, binds to TCP port and returns a Server.
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let connection_pool = get_connection_pool(&configuration.database);
        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address.");
        let timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
            timeout,
        );
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            connection_pool,
            email_client,
            configuration.application.base_url,
        )?;
        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .connect_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.with_db())
}

pub struct ApplicationBaseUrl(pub String);

// Return a Result to the Server, which the caller can .await.
// If we choose to await here, it would be extremely difficult to run this
// function in tokio::spawn (not sure why).
pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
) -> Result<Server, std::io::Error> {
    // App data (e.g. connection) needs to be cloneable. But PgConnection does not have .clone().
    // Instead, wrap the connection in a smart pointer - Data uses Atomic Reference Counter (Arc) internally.
    // Unlike Box, Arc allows multiple ownership of the data. Box does not provide .clone().
    // Arc increments the number of active references for every clone of it.
    let pool = web::Data::new(db_pool);

    // Although EmailClient is cloneable, we want to avoid creating multiple base_url and sender copies.
    // Hence we wrap EmailClient with web::Data, which uses an Arc under-the-hood.
    let email_client = web::Data::new(email_client);

    let base_url = web::Data::new(ApplicationBaseUrl(base_url));

    // HttpServer::new takes a closure instead of an App because it needs to spin up multiple
    // worker processes and provide a different App to each of them.
    // Use `move` to capture `connection` from the surrounding environment. Most useful when passing closure to a new thread so that the new thread owns the data.
    let server = HttpServer::new(move || {
        App::new()
            /*
            tracing_actix_web::TracingLogger is a drop-in replacement for actix_web::middleware::Logger.
            It automatically attaches a unique request_id for each actix-web request.
            */
            .wrap(TracingLogger::default())
            // Routes combines Handlers with a set of Guards
            // "/" implements the Guard trait and passes the request on only if it fulfils.
            // web::get() is short for Route::new().guard(guard::Get()) and passes only GET requests through to the handler
            .route("/health_check", web::get().to(routes::health_check))
            .route("/newsletters", web::post().to(routes::publish_newsletter))
            .route("/subscriptions/confirm", web::get().to(routes::confirm))
            .route("/subscriptions", web::post().to(routes::subscribe))
            .app_data(pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
