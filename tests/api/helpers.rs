use once_cell::sync::Lazy;
use sqlx::{postgres::PgPoolOptions, Connection, Executor, PgConnection, Pool, Postgres};
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::{configuration::{get_configuration, DatabaseSettings}, startup::{get_connection_pool, Application}, telemetry::{get_subscriber, init_subscriber}};

pub struct ConfirmationLinks {
	pub html: String,
	pub plain_text: String,
}

pub struct TestApp {
	pub address: String,
	pub connection_pool: Pool<Postgres>,
	pub email_server: MockServer,
	pub port: u16,
}

impl TestApp {
	pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
		println!("Post Address: {}", &self.address);
		reqwest::Client::new()
			.post(&format!("{}/subscriptions", &self.address))
			.header("Content-Type", "application/x-www-form-urlencoded")
			.body(body)
			.send()
			.await
			.expect("Failed to execute request.")
	}

	pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
		let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

		let get_link = |s: &str| {
			let links: Vec<_> = linkify::LinkFinder::new()
				.links(s)
				.filter(|l| *l.kind() == linkify::LinkKind::Url)
				.collect();
			assert_eq!(links.len(), 1);
			links[0].as_str().to_owned()
		};

		let html = get_link(body["HtmlBody"].as_str().unwrap());
		let plain_text = get_link(body["TextBody"].as_str().unwrap());

		ConfirmationLinks { html, plain_text }
	}
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
	let email_server = MockServer::start().await;

	let config = {
		let mut c = get_configuration().expect("Failed to read configuration");
		c.database.database_name = Uuid::new_v4().to_string();
		c.application.port = 0;
		c.email_client.base_url = email_server.uri();
		c
	};

	configure_database(&config.database).await;

	let app = Application::build(config.clone()).await.expect("Failed to build app.");
	let port = app.port();
	let address = format!("http://127.0.0.1:{}", app.port());
	println!("App Address: {}", address);
	let _fut = tokio::spawn(app.run_until_stopped());

	TestApp {
		address,
		connection_pool: get_connection_pool(config.database),
		email_server,
		port,
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
