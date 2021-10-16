use actix_web::{web, App, HttpRequest, HttpServer, Responder};

// A type implements Responder if it can be converted to HttpResponse
async fn greet(req: HttpRequest) -> impl Responder {
    // Opinion: not too fond of .get("name") not being type-checked against .route("/{name}").
    // Problematic if we change "name" is the handler/route but not the other.
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            // Routes combines Handlers with a set of Guards
            // "/" implements the Guard trait and passes the request on only if it fulfils.
            // web::get() is short for Route::new().guard(guard::Get()) and passes only GET requests through to the handler
            .route("/", web::get().to(greet))
            .route("/{name}", web::get().to(greet))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
