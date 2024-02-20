use actix_web::{web, HttpResponse};
use serde::Deserialize;
use chrono::Utc;
use uuid::Uuid;
use sqlx::{query, Pool, Postgres};

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};

#[derive(Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

#[tracing::instrument(
	name = "Adding a new subscriber",
	skip(form, connection_pool),
	fields(
		subscriber_email = %form.email,
		subscriber_name = %form.name
	)
)]
pub async fn subscribe(form: web::Form<FormData>, connection_pool: web::Data<Pool<Postgres>>) -> HttpResponse {
	let new_subscriber = match form.0.try_into() {
		Ok(subscriber) => subscriber,
		Err(_) => return HttpResponse::BadRequest().finish(),
	};
	match insert_subscriber(&new_subscriber, &connection_pool).await {
		Ok(_) => HttpResponse::Ok().finish(),
		Err(_) => HttpResponse::InternalServerError().finish(),
	}
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
		INSERT INTO subscriptions (id, email, name, subscribed_at)
		VALUES ($1, $2, $3, $4)
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
