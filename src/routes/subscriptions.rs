use actix_web::{web, HttpResponse};
use serde::Deserialize;
use chrono::Utc;
use uuid::Uuid;
use sqlx::{query, Pool, Postgres};

#[derive(Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

pub async fn subscribe(form: web::Form<FormData>, connection_pool: web::Data<Pool<Postgres>>) -> HttpResponse {
	match query!(
		r#"
		INSERT INTO subscriptions (id, email, name, subscribed_at)
		VALUES ($1, $2, $3, $4)
		"#,
		Uuid::new_v4(),
		form.email,
		form.name,
		Utc::now()
	)
	.execute(connection_pool.get_ref())
	.await {
		Ok(_) => HttpResponse::Ok().finish(),
		Err(e) => {
			println!("Failed to execute query: {:?}", e);
			HttpResponse::InternalServerError().finish()
		}
	}
}
