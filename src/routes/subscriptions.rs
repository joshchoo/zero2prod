use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;
use tracing_futures::Instrument;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SubscriberData {
    email: String, // Each argument must implement the FormRequest trait.
    name: String,
}

pub async fn subscribe(
    // Extract form data from x-www-form-urlencoded
    form: web::Form<SubscriberData>,
    // Extract PgConnection from application state
    pool: web::Data<PgPool>,
) -> HttpResponse {
    let request_id = Uuid::new_v4();

    // Logs the Adding a new subscriber.
    let request_span = tracing::info_span!("Adding a new subscriber.", %request_id, subscriber_email = %form.email, subscriber_name = %form.name);
    // Starts and logs the entry into span. When this variable drops, it will exit the span.
    let _request_span_guard = request_span.enter();

    let query_span = tracing::info_span!("Saving new subscriber details in the database");
    match sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at)
    VALUES ($1, $2, $3, $4)
    "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pool.get_ref())
    .instrument(query_span)
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            tracing::error!(
                "request_id {} - Failed to execute query: {:?}",
                request_id,
                e
            );
            HttpResponse::InternalServerError().finish()
        }
    }
}
