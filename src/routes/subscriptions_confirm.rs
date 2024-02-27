use actix_web::{web, HttpResponse};
use sqlx::{pool::Pool, Postgres};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
	pub subscription_token: String,
}

#[tracing::instrument(
	name = "Confirm a pending subscriber",
	skip(parameters),
)]
pub async fn confirm(
	parameters: web::Query<Parameters>,
	pool: web::Data<Pool<Postgres>>,
) -> HttpResponse {
	let id = match get_subscriber_id_from_token(&pool, &parameters.subscription_token).await {
		Ok(id) => id,
		Err(_) => return HttpResponse::BadRequest().finish(),
	};
	match id {
		None => HttpResponse::Unauthorized().finish(),
		Some(subscriber_id) => {
			if confirm_subscriber(&pool, subscriber_id).await.is_err() {
				return HttpResponse::InternalServerError().finish();
			}
			HttpResponse::Ok().finish()
		}
	}
}

#[tracing::instrument(
	name = "Mark a subscriber as confirmed in the database",
	skip(pool, subscriber_id),
)]
pub async fn confirm_subscriber(
	pool: &Pool<Postgres>,
	subscriber_id: Uuid,
) -> Result<(), sqlx::Error> {
	sqlx::query!(
		"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1",
		subscriber_id
	)
	.execute(pool)
	.await
	.map_err(|e| {
		tracing::error!("Failed to execute query: {:?}", e);
		e
	})?;
	Ok(())
}

#[tracing::instrument(
	name = "Retrieve subscriber ID by token from the database",
	skip(pool, subscription_token),
)]
pub async fn get_subscriber_id_from_token(
	pool: &Pool<Postgres>,
	subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
	let result = sqlx::query!(
		"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1",
		subscription_token
	)
	.fetch_optional(pool)
	.await
	.map_err(|e| {
		tracing::error!("Failed to execute query: {:?}", e);
		e
	})?;
	Ok(result.map(|r| r.subscriber_id))
}