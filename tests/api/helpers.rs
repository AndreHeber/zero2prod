use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use sqlx::{postgres::PgPoolOptions, Connection, Executor, PgConnection, Pool, Postgres};
use testcontainers::{clients::Cli, RunnableImage};
use uuid::Uuid;
use std:: net::TcpListener;
use zero2prod::{configuration::{get_configuration, DatabaseSettings}, email_client, telemetry::{get_subscriber, init_subscriber}};

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

fn create_db(config: &DatabaseSettings) -> RunnableImage<testcontainers_modules::postgres::Postgres> {
	RunnableImage::from(testcontainers_modules::postgres::Postgres::default())
		.with_env_var(("POSTGRES_PASSWORD", config.password.expose_secret()))
		.with_env_var(("POSTGRES_USER", &config.username))
		.with_env_var(("POSTGRES_DB", &config.database_name))
}

pub async fn spawn_app() -> TestApp {
	Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

	let mut config = get_configuration().expect("Failed to read configuration");
	let image = create_db(&config.database);
	let docker = Cli::default();
	docker.run(image);
	config.database.database_name = Uuid::new_v4().to_string();
	let connection_pool = configure_database(&config.database).await;

	let sender_email = config.email_client.sender().unwrap();
	let timeout = config.email_client.timeout();
	let email_client = email_client::EmailClient::new(
		config.email_client.base_url,
		sender_email,
		config.email_client.authorization_token,
		timeout,
	);

    let server = zero2prod::startup::run(listener, connection_pool.clone(), email_client).expect("Failed to bind address");
    let _fut = tokio::spawn(server);

	TestApp {
		address,
		connection_pool,
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
