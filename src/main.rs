use std::net::TcpListener;
use sqlx::postgres::PgPoolOptions;
use zero2prod::configuration::get_configuration;
use zero2prod::email_client;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
	let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
	init_subscriber(subscriber);

    let config = get_configuration().expect("Failed to read configuration");

	let connection_pool = PgPoolOptions::new().acquire_timeout(std::time::Duration::from_secs(2))
    .max_connections(10).connect_lazy_with(config.database.with_db());

	let sender_email = config.email_client.sender().unwrap();
	let timeout = config.email_client.timeout();
	let email_client = email_client::EmailClient::new(
		config.email_client.base_url,
		sender_email,
		config.email_client.authorization_token,
		timeout,
	);

    let address = format!("{}:{}", config.application.host, config.application.port);
    let listener = TcpListener::bind(address).expect("Failed to bind random port");
    run(listener, connection_pool ,email_client)?.await
}
