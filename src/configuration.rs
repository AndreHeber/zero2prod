use config::Config;
use secrecy::{ExposeSecret, Secret};
use sqlx::{postgres::{PgConnectOptions, PgSslMode}, ConnectOptions};

use crate::domain::SubscriberEmail;

#[derive(serde::Deserialize)]
pub struct Settings {
	pub database: DatabaseSettings,
	pub application: ApplicationSettings,
	pub email_client: EmailClientSettings,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
	pub port: u16,
	pub host: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
	pub username: String,
	pub password: Secret<String>,
	pub port: u16,
	pub host: String,
	pub database_name: String,
	pub require_ssl: bool,
}

impl DatabaseSettings {
	pub fn without_db(&self) -> PgConnectOptions {
		let ssl_mode = if self.require_ssl {
			PgSslMode::Require
		} else {
			PgSslMode::Prefer
		};
		PgConnectOptions::new()
			.host(&self.host)
			.username(&self.username)
			.password(self.password.expose_secret())
			.port(self.port)
			.ssl_mode(ssl_mode)
	}

	pub fn with_db(&self) -> PgConnectOptions {
		let options = self.without_db().database(&self.database_name);
		options.log_statements(tracing::log::LevelFilter::Trace)
	}
}

#[derive(serde::Deserialize)]
pub struct EmailClientSettings {
	pub base_url: String,
	pub sender_email: String,
	pub authorization_token: Secret<String>,
	pub timeout_milliseconds: u64,
}

impl EmailClientSettings {
	pub fn sender(&self) -> Result<SubscriberEmail, String> {
		SubscriberEmail::parse(self.sender_email.clone())
	}
	pub fn timeout(&self) -> std::time::Duration {
		std::time::Duration::from_millis(self.timeout_milliseconds)
	}
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
	let run_mode = std::env::var("APP_ENVIRONMENT").unwrap_or("development".into());
	let base_path = std::env::current_dir().expect("Failed to determine the current directory");
	let configuration_directory = base_path.join("configuration"); // "./configuration"

	let base_config_file = configuration_directory.join("base"); // "./configuration/base"
	let base_config_file = base_config_file.to_str().expect("Invalid base config file path");
	let environment_config_file = configuration_directory.join(run_mode.to_lowercase()); // "./configuration/[development/production]"
	let environment_config_file = environment_config_file.to_str().expect("Invalid environment config file path");

	let settings = Config::builder()
		.add_source(config::File::with_name(base_config_file).required(true))
		.add_source(config::File::with_name(environment_config_file).required(true))
		.add_source(config::Environment::with_prefix("APP").try_parsing(true).separator("_"))
		.build()?;

	settings.try_deserialize()
}

pub enum Environment {
	Development,
	Production,
}

impl Environment {
	pub fn as_str(&self) -> &'static str {
		match self {
			Environment::Development => "development",
			Environment::Production => "production",
		}
	}
}

impl TryFrom<String> for Environment {
	type Error = String;

	fn try_from(s: String) -> Result<Self, Self::Error> {
		match s.to_lowercase().as_str() {
			"development" => Ok(Environment::Development),
			"production" => Ok(Environment::Production),
			_ => Err(format!("{} is not a valid environment", s)),
		}
	}
}