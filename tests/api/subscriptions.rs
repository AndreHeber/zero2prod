use crate::helpers::spawn_app;
use sqlx::query;
use wiremock::{matchers::{path, method}, Mock, ResponseTemplate, http::Method};

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let app = spawn_app().await;
	Mock::given(path("/email"))
		.and(method(Method::POST))
		.respond_with(ResponseTemplate::new(200))
		.expect(1)
		.mount(&app.email_server)
		.await;
	let response = app.post_subscriptions("name=andre&email=andre.heber@gmx.net".to_string()).await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    let app = spawn_app().await;
	Mock::given(path("/email"))
		.and(method(Method::POST))
		.respond_with(ResponseTemplate::new(200))
		.expect(1)
		.mount(&app.email_server)
		.await;
	let _response = app.post_subscriptions("name=andre&email=andre.heber@gmx.net".to_string()).await;

	let saved = query!("SELECT email, name, status FROM subscriptions",)
		.fetch_one(&app.connection_pool)
		.await
		.expect("Failed to fetch saved subscription.");

	assert_eq!(saved.email, "andre.heber@gmx.net");
	assert_eq!(saved.name, "andre");
	assert_eq!(saved.status, "pending_confirmation");
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    let app = spawn_app().await;
    for (invalid_body, error_message) in test_cases {
		let response = app.post_subscriptions(invalid_body.to_string()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_invalid() {
	let test_cases = vec![
		("name=&email=ursula_le_guin%40gmail.com", "name is empty"),
		("name=le%20guin&email=", "email is empty"),
		("name=&email=", "name and email are empty"),
		("name=Ursula&email=definitely-not-an-email", "invalid email"),
	];

	let app = spawn_app().await;
	for (invalid_body, error_message) in test_cases {
		let response = app.post_subscriptions(invalid_body.to_string()).await;

		assert_eq!(
			400,
			response.status().as_u16(),
			"The API did not return a 400 Bad Request when the payload was {}.",
			error_message
		);
	}
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
	let app = spawn_app().await;

	let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

	Mock::given(path("/email"))
		.and(method(Method::POST))
		.respond_with(ResponseTemplate::new(200))
		.expect(1)
		.mount(&app.email_server)
		.await;

	let _response = app.post_subscriptions(body.to_string()).await;
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
	let app = spawn_app().await;

	let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

	Mock::given(path("/email"))
		.and(method(Method::POST))
		.respond_with(ResponseTemplate::new(200))
		.expect(1)
		.mount(&app.email_server)
		.await;

	let response = app.post_subscriptions(body.to_string()).await;

	assert_eq!(200, response.status().as_u16());

	let email_request = &app.email_server.received_requests().await.unwrap()[0];
	let confirmation_links = app.get_confirmation_links(email_request);

	// The two links should be identical
	assert_eq!(confirmation_links.html, confirmation_links.plain_text);
}