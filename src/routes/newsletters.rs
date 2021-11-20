use actix_http::{
    header::{self, HeaderMap, HeaderValue},
    StatusCode,
};
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use sqlx::PgPool;

use crate::{domain::SubscriberEmail, email_client::EmailClient, routes::error_chain_fmt};

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

struct Credentials {
    #[allow(dead_code)]
    username: String,
    #[allow(dead_code)]
    password: String,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header is missing.")?
        .to_str()
        .context("The 'Authorization' header is not a valid UTF-8 string.")?;
    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authentication scheme is not 'Basic'.")?;
    let decoded_bytes = base64::decode(base64encoded_segment)
        .context("Failed to base64-decode 'Basic' credentials.")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF-8.")?;

    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?
        .to_string();

    Ok(Credentials { username, password })
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    // expected_password_hash is stored in PHC string format: "${algorithm}${algorithm version}${$-separated algorithm parameters}${hash}${salt}"
    let (user_id, expected_password_hash_phc) = get_stored_credentials(&credentials.username, pool)
        .await
        .map_err(PublishError::UnexpectedError)?
        // Using ok_or_else converts the Option to Result and makes it convenient to propagate any Err with `?`.
        .ok_or_else(|| PublishError::AuthError(anyhow::anyhow!("Unknown username.")))?;

    let current_span = tracing::Span::current();
    // Move CPU-intensive hashing to a separate thread
    actix_web::rt::task::spawn_blocking(move || {
        // tracing::info_span!("Verify password hash")
        //     .in_scope(|| verify_password_hash(expected_password_hash_phc, credentials.password))
        current_span
            .in_scope(|| verify_password_hash(expected_password_hash_phc, credentials.password))
    })
    .await
    .context("failed to spawn blocking task.")
    .map_err(PublishError::UnexpectedError)??;

    Ok(user_id)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash_phc, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash_phc: String,
    password_candidate: String,
) -> Result<(), PublishError> {
    let expected_password_hash = PasswordHash::new(&expected_password_hash_phc)
        .context("Failed to parse hash in PHC string format")
        .map_err(PublishError::UnexpectedError)?;

    // Execute the function within the scope of this span.
    Argon2::default()
        .verify_password(password_candidate.as_bytes(), &expected_password_hash)
        .context("Invalid password")
        .map_err(PublishError::AuthError)
}

#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, String)>, anyhow::Error> {
    let row: Option<_> = sqlx::query!(
        r#"
        SELECT user_id, password_hash
        FROM users
        WHERE username = $1
        "#,
        username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve stored credentials.")?
    .map(|row| (row.user_id, row.password_hash));

    Ok(row)
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    // skip(body, pool, email_client, request),
    skip_all,
    // fields: Specify empty username and user_id fields that we will manually record later using `tracing::Span::current().record("...", ...)`
    // tracing::field::Empty - Indicates that the value of a field is not currently present but might be recorded later.
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: web::HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));
    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

    let subscribers = get_confirmed_subscribers(&pool).await?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    // unlike `context`, `with_context` is lazy, which avoids the runtime cost of format! heap allocation
                    .with_context(|| {
                        // format! allocates memory on the heap for the output string
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid."
                )
            }
        };
    }
    Ok(HttpResponse::Ok().finish())
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    // struct Row {
    //     email: String,
    // }

    // // query_as! Maps the retrieved rows to the ConfirmedSubscriber struct
    // let rows = sqlx::query_as!(
    //     Row,
    //     r#"
    //     SELECT email
    //     FROM subscriptions
    //     WHERE status = 'confirmed'
    //     "#
    // )
    // .fetch_all(pool)
    // .await?;

    let rows = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#
    )
    .fetch_all(pool)
    .await?;

    let confirmed_subscribers = rows
        .into_iter()
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();
    Ok(confirmed_subscribers)
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    // Only one variant can use #[from] for the same wrapped data type. In this case, anyhow::Errors propagated by "?" will be transformed to UnexpectedError.
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
        }
    }

    // If we implement `error_response`, we don't need to implement `status_code`
    //
    // fn status_code(&self) -> actix_http::StatusCode {
    //     match self {
    //         PublishError::AuthError(_) => StatusCode::UNAUTHORIZED,
    //         PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
    //     }
    // }
}
