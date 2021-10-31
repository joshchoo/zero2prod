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

    match insert_subscriber(&pool, &form).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn insert_subscriber(
    pool: &PgPool,
    subscriber: &SubscriberData,
) -> Result<(), sqlx::Error> {
    let query_span = tracing::info_span!("Saving new subscriber details in the database");
    sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at)
    VALUES ($1, $2, $3, $4)
    "#,
        Uuid::new_v4(),
        subscriber.email,
        subscriber.name,
        Utc::now()
    )
    .execute(pool)
    .instrument(query_span)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}
