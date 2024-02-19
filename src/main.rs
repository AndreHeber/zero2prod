use std::net::TcpListener;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
	let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
	init_subscriber(subscriber);

    let config = get_configuration().expect("Failed to read configuration");
	let connection_pool = PgPoolOptions::new()
    .max_connections(10).connect(config.database.connection_string().expose_secret()).await.expect("Failed to connect to Postgres.");

    let address = format!("127.0.0.1:{}", config.application_port);
    let listener = TcpListener::bind(address).expect("Failed to bind random port");
    run(listener, connection_pool)?.await
}
