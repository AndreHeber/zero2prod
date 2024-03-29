use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::{Pool, Postgres};
use tracing_actix_web::TracingLogger;
use std::net::TcpListener;

use crate::email_client::EmailClient;
use crate::routes::{confirm, health_check, subscribe};

pub fn run(
	listener: TcpListener,
	connection_pool: Pool<Postgres>,
	email_client: EmailClient,
	base_url: String,
) -> Result<Server, std::io::Error> {
	let connection_pool = web::Data::new(connection_pool);
	let email_client = web::Data::new(email_client);
	let base_url = web::Data::new(ApplicationBaseUrl(base_url));
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .app_data(connection_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}

pub struct Application {
	pub port: u16,
	pub server: Server,
}

impl Application {
	pub async fn build(
		config: crate::configuration::Settings,
	) -> Result<Self, std::io::Error> {
		let connection_pool: Pool<Postgres> = get_connection_pool(config.database);
	
		let sender_email = config.email_client.sender().unwrap();
		let timeout = config.email_client.timeout();
		let email_client = EmailClient::new(
			config.email_client.base_url,
			sender_email,
			config.email_client.authorization_token,
			timeout,
		);
	
		let listener = TcpListener::bind(format!("{}:{}", config.application.host, config.application.port))?;
		let port = listener.local_addr().unwrap().port();
		let server = run(listener, connection_pool, email_client, config.application.base_url)?;
		Ok(Self { port, server })
	}

	pub fn port(&self) -> u16 {
		self.port
	}

	pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
		self.server.await
	}
}

pub struct ApplicationBaseUrl(pub String);

pub fn get_connection_pool(config: crate::configuration::DatabaseSettings) -> Pool<Postgres> {
	Pool::connect_lazy_with(config.with_db())
}