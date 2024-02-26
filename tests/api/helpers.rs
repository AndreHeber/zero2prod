use once_cell::sync::Lazy;
use sqlx::{postgres::PgPoolOptions, Connection, Executor, PgConnection, Pool, Postgres};
use uuid::Uuid;
use zero2prod::{configuration::{get_configuration, DatabaseSettings}, startup::{get_connection_pool, Application}, telemetry::{get_subscriber, init_subscriber}};

pub struct TestApp {
	pub address: String,
	pub connection_pool: Pool<Postgres>,
}

static TRACING: Lazy<()> = Lazy::new(|| {
	let default_filter_level = "info".to_string();
	let subscriber_name = "test".to_string();

	if std::env::var("TEST_LOG").is_ok() {
		let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
		init_subscriber(subscriber);
	} else {
		let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
		init_subscriber(subscriber);
	}
});

pub async fn spawn_app() -> TestApp {
	Lazy::force(&TRACING);

	let config = {
		let mut c = get_configuration().expect("Failed to read configuration");
		c.database.database_name = Uuid::new_v4().to_string();
		c.application.port = 0;
		c
	};

	configure_database(&config.database).await;

	let app = Application::build(config.clone()).await.expect("Failed to build app.");
	let address = format!("http://127.0.0.1:{}", app.port());
	let _fut = tokio::spawn(app.run_until_stopped());

	TestApp {
		address,
		connection_pool: get_connection_pool(config.database),
	}
}

pub async fn configure_database(config: &DatabaseSettings) -> Pool<Postgres> {
	let mut connection = PgConnection::connect_with(&config.without_db())
		.await
		.expect("Failed to connect to Postgres.");

		connection.execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
		.await
		.expect("Failed to create database.");

	let connection_pool = PgPoolOptions::new()
		.max_connections(10)
		.connect_with(config.with_db())
		.await
		.expect("Failed to connect to Postgres.");

	sqlx::migrate!("./migrations")
		.run(&connection_pool)
		.await
		.expect("Failed to run migrations.");

	connection_pool
}
