use actix_web::{web, HttpResponse, ResponseError};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Deserialize;
use chrono::Utc;
use uuid::Uuid;
use sqlx::{query, Pool, Postgres, Transaction};
use lettre::{
    address::AddressError, message::{header::ContentType, Mailbox}, transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport
};

use crate::{domain::{NewSubscriber, SubscriberEmail, SubscriberName}, email_client::EmailClient, startup::ApplicationBaseUrl};

#[derive(Debug)]
pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "A database error was encountered while trying to store the subscription token.")
	}
}

impl ResponseError for StoreTokenError {}

#[derive(Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

#[tracing::instrument(
	name = "Adding a new subscriber",
	skip(form, connection_pool, email_client, base_url),
	fields(
		subscriber_email = %form.email,
		subscriber_name = %form.name
	)
)]
pub async fn subscribe(
	form: web::Form<FormData>,
	connection_pool: web::Data<Pool<Postgres>>,
	email_client: web::Data<EmailClient>,
	base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, actix_web::Error> {
	let new_subscriber = match form.0.try_into() {
		Ok(subscriber) => subscriber,
		Err(e) => return Err(actix_web::error::ErrorBadRequest(e)),
	};
	let mut transaction = match connection_pool.begin().await {
		Ok(transaction) => transaction,
		Err(_) => return Err(actix_web::error::ErrorInternalServerError("Failed to acquire a database connection.")),
	};
	let subscriber_id = match insert_subscriber(&new_subscriber, &mut transaction).await {
		Ok(subscriber_id) => subscriber_id,
		Err(_) => return Err(actix_web::error::ErrorInternalServerError("Failed to save new subscriber details.")),
	};
	let subscription_token = generate_confirmation_token();
	store_token(&mut transaction, &subscriber_id, &subscription_token).await?;
	if transaction.commit().await.is_err() {
		return HttpResponse::InternalServerError().finish();
	}
	if send_confirmation_email(&email_client, new_subscriber, &base_url.0, &subscription_token).await.is_err() {
		return HttpResponse::InternalServerError().finish();
	}
	HttpResponse::Ok().finish()
}

#[tracing::instrument(
	name = "Send a confirmation email to the new subscriber",
	skip(email_client, new_subscriber, base_url)
)]
pub async fn send_confirmation_email(
	email_client: &EmailClient,
	new_subscriber: NewSubscriber,
	base_url: &str,
	subscription_token: &str,
) -> Result<(), reqwest::Error> {
	let confirmation_link = format!("{}/subscriptions/confirm?subscription_token={}", base_url, subscription_token);
	let plain_body = &format!(
		"Welcome to our newsletter!\n\
		Visit {} to confirm your subscription.",
		confirmation_link
	);
	let html_body = &format!(
		"Welcome to our newsletter!<br />\
		Click <a href=\"{}\">here</a> to confirm your subscription.",
		confirmation_link
	);
	email_client
		.send_email(
			new_subscriber.email,
			"Welcome!",
			html_body,
			plain_body,
		)
		.await
}

#[tracing::instrument(
	name = "Sending confirmation email",
	skip(new_subscriber)
)]
fn send_mail(new_subscriber: &NewSubscriber) -> Result<(), String> {
	let from: Mailbox = "Andre Heber <andre@futureblog.eu>".parse().map_err(|e: AddressError| {
		tracing::error!("Could not parse email - from: {:?}", e);
		e.to_string()
	})?;

	let reply_to: Mailbox = "noreply@futureblog.eu".parse().map_err(|e: AddressError| {
		tracing::error!("Could not parse email - reply_to: {:?}", e);
		e.to_string()
	})?;

	let to: Mailbox = new_subscriber.email.as_ref().parse().map_err(|e: AddressError| {
		tracing::error!("Could not parse email - to: {:?}", e);
		e.to_string()
	})?;

	let email = Message::builder()
		.from(from)
		.reply_to(reply_to)
		.to(to)
		.subject("Rust Email")
		.header(ContentType::TEXT_PLAIN)
		.body(String::from("Hello, this is a test email from Rust!"))
		.map_err(|e| {
			tracing::error!("Could not build email: {:?}", e);
			e.to_string()
		})?;

	let creds = Credentials::new("andre@futureblog.eu".to_string(), "d6vanPc4RUeQ".to_string());
	let mailer = SmtpTransport::relay("mail.futureblog.eu")
    	.map_err(|e| {
			tracing::error!("Could not connect to server: {:?}", e);
			e.to_string()
		})?;
	let mailer = mailer.credentials(creds).build();

	mailer.send(&email)
		.map_err(|e| {
			tracing::error!("Could not send email: {:?}", e);
			e.to_string()
		})?;
	Ok(())
}

impl TryFrom<FormData> for NewSubscriber {
	type Error = String;

	fn try_from(form: FormData) -> Result<NewSubscriber, Self::Error> {
		let name = SubscriberName::parse(form.name)?;
		let email = SubscriberEmail::parse(form.email)?;
		Ok(NewSubscriber { email, name })
	}
}

#[tracing::instrument(
	name = "Saving new subscriber details in the database",
	skip(new_subscriber, transaction)
)]
async fn insert_subscriber(new_subscriber: &NewSubscriber, transaction: &mut Transaction<'_, Postgres>) -> Result<Uuid, sqlx::Error> {
	let subscriber_id = Uuid::new_v4();
	query!(
		r#"
		INSERT INTO subscriptions (id, email, name, subscribed_at, status)
		VALUES ($1, $2, $3, $4, 'pending_confirmation')
		"#,
		subscriber_id,
		new_subscriber.email.as_ref(),
		new_subscriber.name.as_ref(),
		Utc::now()
	)
	.execute(&mut **transaction)
	.await
	.map_err(|e| {
		tracing::error!("Failed to execute query: {:?}", e);
		e
	})?;
	Ok(subscriber_id)
}

#[tracing::instrument(
	name = "Storing subscription token in the database",
	skip(transaction, subscriber_id, subscription_token)
)]
async fn store_token(transaction: &mut Transaction<'_, Postgres>, subscriber_id: &Uuid, subscription_token: &str) -> Result<(), StoreTokenError> {
	query!(
		r#"
		INSERT INTO subscription_tokens (subscription_token, subscriber_id)
		VALUES ($1, $2)
		"#,
		subscription_token,
		subscriber_id
	)
	.execute(&mut **transaction)
	.await
	.map_err(|e| {
		tracing::error!("Failed to execute query: {:?}", e);
		StoreTokenError(e)
	})?;
	Ok(())
}

fn generate_confirmation_token() -> String {
	let mut rng = thread_rng();
	std::iter::repeat_with(|| rng.sample(Alphanumeric))
		.map(char::from)
		.take(25)
		.collect()
}