use crate::{domain::NewSubscriber, email_client::EmailClient};
use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;
use std::convert::TryInto;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SubscriberData {
    pub email: String, // Each argument must implement the FormRequest trait.
    pub name: String,
}

// Clippy currently detects an issue between tracing::instrument and an actix_web handler: https://github.com/tokio-rs/tracing/issues/1450
#[allow(clippy::async_yields_async)]
#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool, email_client),
    // Inject the following fields into all spans of the request
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    // Extract form data from x-www-form-urlencoded
    form: web::Form<SubscriberData>,
    // Extract PgConnection from application state
    pool: web::Data<PgPool>,
    // Extract EmailClient from application state
    email_client: web::Data<EmailClient>,
) -> HttpResponse {
    let new_subscriber: NewSubscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    if insert_subscriber(&pool, &new_subscriber).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    if email_client
        .send_email(
            new_subscriber.email,
            "Welcome!",
            "Welcome to our newsletter!",
            "<p>Welcome to our newsletter!</p>",
        )
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    };

    HttpResponse::Ok().finish()
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database"
    skip(pool, new_subscriber)
)]
pub async fn insert_subscriber(
    pool: &PgPool,
    new_subscriber: &NewSubscriber,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at, status)
    VALUES ($1, $2, $3, $4, $5)
    "#,
        Uuid::new_v4(),
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
        "confirmed"
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}
