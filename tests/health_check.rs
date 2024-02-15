use sqlx::{postgres::PgPoolOptions, query, Connection, PgConnection, Pool, Postgres};
use std::net::TcpListener;

use zero2prod::configuration::get_configuration;

fn spawn_app(connection: Pool<Postgres>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();

    let server = zero2prod::startup::run(listener, connection).expect("Failed to bind address");
    tokio::spawn(server);

    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn health_check_works() {
	let config = get_configuration().expect("Failed to read configuration");
	let connection = PgPoolOptions::new()
    .max_connections(10).connect(&config.database.connection_string()).await.expect("Failed to connect to Postgres.");

    let address = spawn_app(connection);
    let health_check_endpoint = format!("{}/health_check", address);
    let client = reqwest::Client::new();
    let response = client
        .get(health_check_endpoint)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
	let config = get_configuration().expect("Failed to read configuration");
	let connection = PgPoolOptions::new()
    .max_connections(10).connect(&config.database.connection_string()).await.expect("Failed to connect to Postgres.");

    let app_address = spawn_app(connection);

	let mut connection = PgConnection::connect(&config.database.connection_string()).await.expect("Failed to connect to Postgres.");

    let client = reqwest::Client::new();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app_address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, response.status().as_u16());

	let saved = query!("SELECT email, name FROM subscriptions",)
		.fetch_one(&mut connection)
		.await
		.expect("Failed to fetch saved subscription.");

	assert_eq!(saved.email, "ursula_le_guin@gmail.com");
	assert_eq!(saved.name, "le guin");
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
	let config = get_configuration().expect("Failed to read configuration");
	let connection = PgPoolOptions::new()
    .max_connections(10).connect(&config.database.connection_string()).await.expect("Failed to connect to Postgres.");

    let app_address = spawn_app(connection);
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &app_address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}
