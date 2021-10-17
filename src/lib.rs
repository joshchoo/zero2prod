use actix_web::dev::Server;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use std::net::TcpListener;

// Return a Result to the Server, which the caller can .await.
// If we choose to await here, it would be extremely difficult to run this
// function in tokio::spawn (not sure why).
pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            // Routes combines Handlers with a set of Guards
            // "/" implements the Guard trait and passes the request on only if it fulfils.
            // web::get() is short for Route::new().guard(guard::Get()) and passes only GET requests through to the handler
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
    })
    .listen(listener)?
    .run();
    Ok(server)
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}

#[derive(Deserialize)]
struct SubscriberData {
    email: String, // Each argument must implement the FormRequest trait.
    name: String,
}

// Form extractor for x-www-form-urlencoded data
async fn subscribe(_form: web::Form<SubscriberData>) -> HttpResponse {
    HttpResponse::Ok().finish()
}
