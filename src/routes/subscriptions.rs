use crate::{domain::NewSubscriber, email_client::EmailClient, startup::ApplicationBaseUrl};
use actix_http::StatusCode;
use actix_web::{web, HttpResponse, ResponseError};
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, Transaction};
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
    skip(form, pool, email_client, base_url),
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
    base_url: web::Data<ApplicationBaseUrl>,
    // SubscribeError implements the needed actix_web::ResponseError
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber: NewSubscriber =
        form.0.try_into().map_err(SubscribeError::ValidationError)?;
    let mut transaction = pool.begin().await.map_err(SubscribeError::PoolError)?;
    let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .map_err(SubscribeError::InsertSubscriberError)?;
    let subscription_token = generate_subscription_token();
    store_token(&mut transaction, subscriber_id, &subscription_token).await?;
    transaction
        .commit()
        .await
        .map_err(SubscribeError::TransactionCommitError)?;
    send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let html_body = format!(
        "Welcome to our newsletter!<br />\
    Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );
    let plain_text_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    email_client
        .send_email(
            new_subscriber.email,
            "Welcome!",
            &plain_text_body,
            &html_body,
        )
        .await
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database"
    skip(transaction, new_subscriber)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at, status)
    VALUES ($1, $2, $3, $4, $5)
    "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
        "pending_confirmation"
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(subscriber_id)
}

fn generate_subscription_token() -> String {
    let rng = thread_rng();
    rng.sample_iter(Alphanumeric)
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "Saving subscription token in the database",
    skip(transaction, subscriber_id, subscription_token)
)]
async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        "INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES($1, $2)",
        subscription_token,
        subscriber_id
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        StoreTokenError(e)
    })?;
    Ok(())
}

/// SubscribeError represents all the errors that could happen during subscription.
#[derive(thiserror::Error)] // This helps us implement `std::error::Error`, `std::fmt::Display`
pub enum SubscribeError {
    #[error("{0}")] // Interpolates the inner String as the Display value
    ValidationError(String),
    // The error macro is used for the Display implementation
    #[error("Failed to store the confirmation token for a new subscriber.")]
    StoreTokenError(#[from] StoreTokenError), // #[from] also acts as #[source] implicitly
    #[error("Failed to send a confirmation email.")]
    SendEmailError(#[from] reqwest::Error),
    #[error("Failed to acquire a Postgres connection from the pool.")]
    PoolError(#[source] sqlx::Error),
    #[error("Failed to insert a new subscriber in the database.")]
    InsertSubscriberError(#[source] sqlx::Error),
    #[error("Failed to commit SQL transaction to store a new subscriber.")]
    TransactionCommitError(#[source] sqlx::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    // The default status code will be InternalServerError if `ResponseError::status_code` isn't implemented.
    fn status_code(&self) -> actix_http::StatusCode {
        match self {
            Self::ValidationError(_) => StatusCode::BAD_REQUEST,
            Self::StoreTokenError(_)
            | Self::SendEmailError(_)
            | Self::PoolError(_)
            | Self::InsertSubscriberError(_)
            | Self::TransactionCommitError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// Automatically implemented using `thiserror`
//
// impl std::fmt::Display for SubscribeError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::ValidationError(e) => write!(f, "{}", e),
//             Self::StoreTokenError(_) => write!(
//                 f,
//                 "Failed to store the confirmation token for a new subscriber."
//             ),
//             Self::SendEmailError(_) => {
//                 write!(f, "Failed to send a confirmation email.")
//             }
//             Self::PoolError(_) => {
//                 write!(f, "Failed to acquire a Postgres connection from the pool.")
//             }
//             Self::InsertSubscriberError(_) => {
//                 write!(f, "Failed to insert a new subscriber in the database.")
//             }
//             Self::TransactionCommitError(_) => {
//                 write!(
//                     f,
//                     "Failed to commit SQL transaction to store a new subscriber."
//                 )
//             }
//         }
//     }
// }
//
// Implemented by the #[source] and #[from] attributes
//
// impl std::error::Error for SubscribeError {
//     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
//         match self {
//             Self::ValidationError(_) => None,
//             Self::StoreTokenError(e) => Some(e),
//             Self::SendEmailError(e) => Some(e),
//             Self::PoolError(e) => Some(e),
//             Self::InsertSubscriberError(e) => Some(e),
//             Self::TransactionCommitError(e) => Some(e),
//         }
//     }
// }

// Implemented by the #[from] attribute
//
// Implementing `From` for each enum allows automatic type conversion when propagating with `?`.
// impl From<reqwest::Error> for SubscribeError {
//     fn from(e: reqwest::Error) -> Self {
//         Self::SendEmailError(e)
//     }
// }

// impl From<StoreTokenError> for SubscribeError {
//     fn from(e: StoreTokenError) -> Self {
//         Self::StoreTokenError(e)
//     }
// }

pub struct StoreTokenError(sqlx::Error);

// exception.message
impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while \
        trying to store a subscription token."
        )
    }
}

// exception.details
impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // We can pass self because StoreTokenError implements std::error::Error
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // The inner sqlx::Error already implements std::error::Error
        Some(&self.0)
    }
}

/// Iterate over the entire chain of errors.
fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    // This line invokes the `Display.fmt` method on the error `e`
    writeln!(f, "{}\n", e)?;
    // `source` allows us to identify the error cause in the callstack
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
