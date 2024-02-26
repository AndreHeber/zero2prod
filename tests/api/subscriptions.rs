use crate::helpers::spawn_app;
use sqlx::query;

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let app = spawn_app().await;
	let response = app.post_subscriptions("name=andre&email=andre.heber@gmx.net".to_string()).await;

    assert_eq!(200, response.status().as_u16());

	let saved = query!("SELECT email, name FROM subscriptions",)
		.fetch_one(&app.connection_pool)
		.await
		.expect("Failed to fetch saved subscription.");

	assert_eq!(saved.email, "andre.heber@gmx.net");
	assert_eq!(saved.name, "andre");
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
