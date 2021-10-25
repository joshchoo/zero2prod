use crate::routes;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;
use std::net::TcpListener;

// Return a Result to the Server, which the caller can .await.
// If we choose to await here, it would be extremely difficult to run this
// function in tokio::spawn (not sure why).
pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    // App data (e.g. connection) needs to be cloneable. But PgConnection does not have .clone().
    // Instead, wrap the connection in a smart pointer - Data uses Atomic Reference Counter (Arc) internally.
    // Unlike Box, Arc allows multiple ownership of the data. Box does not provide .clone().
    // Arc increments the number of active references for every clone of it.
    let pool = web::Data::new(db_pool);

    // HttpServer::new takes a closure instead of an App because it needs to spin up multiple
    // worker processes and provide a different App to each of them.
    // Use `move` to capture `connection` from the surrounding environment. Most useful when passing closure to a new thread so that the new thread owns the data.
    let server = HttpServer::new(move || {
        App::new()
            // Routes combines Handlers with a set of Guards
            // "/" implements the Guard trait and passes the request on only if it fulfils.
            // web::get() is short for Route::new().guard(guard::Get()) and passes only GET requests through to the handler
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscribe))
            .app_data(pool.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
