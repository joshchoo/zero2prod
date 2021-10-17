use actix_web::{web, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SubscriberData {
    email: String, // Each argument must implement the FormRequest trait.
    name: String,
}

// Form extractor for x-www-form-urlencoded data
pub async fn subscribe(_form: web::Form<SubscriberData>) -> HttpResponse {
    HttpResponse::Ok().finish()
}
