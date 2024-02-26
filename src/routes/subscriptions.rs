use actix_web::{web, HttpResponse};
use serde::Deserialize;
use chrono::Utc;
use uuid::Uuid;
use sqlx::{query, Pool, Postgres};
use lettre::{
    address::AddressError, message::{header::ContentType, Mailbox}, transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport
};

use crate::{domain::{NewSubscriber, SubscriberEmail, SubscriberName}, email_client::EmailClient};

#[derive(Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

#[tracing::instrument(
	name = "Adding a new subscriber",
	skip(form, connection_pool, email_client),
	fields(
		subscriber_email = %form.email,
		subscriber_name = %form.name
	)
)]
pub async fn subscribe(
	form: web::Form<FormData>,
	connection_pool: web::Data<Pool<Postgres>>,
	email_client: web::Data<EmailClient>,
) -> HttpResponse {
	let new_subscriber = match form.0.try_into() {
		Ok(subscriber) => subscriber,
		Err(_) => return HttpResponse::BadRequest().finish(),
	};
	match insert_subscriber(&new_subscriber, &connection_pool).await {
		Ok(_) => (),
		Err(_) => return HttpResponse::InternalServerError().finish(),
	}
	if send_confirmation_email(&email_client, new_subscriber).await.is_err() {
		return HttpResponse::InternalServerError().finish();
	}
	HttpResponse::Ok().finish()
}

#[tracing::instrument(
	name = "Send a confirmation email to the new subscriber",
	skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(
	email_client: &EmailClient,
	new_subscriber: NewSubscriber,
) -> Result<(), reqwest::Error> {
	let confirmation_link = "https://my-api.com/subscriptions/confirm";
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
	skip(new_subscriber, connection_pool)
)]
async fn insert_subscriber(new_subscriber: &NewSubscriber, connection_pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
	query!(
		r#"
		INSERT INTO subscriptions (id, email, name, subscribed_at, status)
		VALUES ($1, $2, $3, $4, 'pending_confirmation')
		"#,
		Uuid::new_v4(),
		new_subscriber.email.as_ref(),
		new_subscriber.name.as_ref(),
		Utc::now()
	)
	.execute(connection_pool)
	.await
	.map_err(|e| {
		tracing::error!("Failed to execute query: {:?}", e);
		e
	})?;
	Ok(())
}
