use reqwest::Url;
use wiremock::{matchers::{method, path}, Mock, ResponseTemplate};

use crate::helpers::spawn_app;

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
	let app = spawn_app().await;
	let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address)).await.unwrap();
	assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
	let app = spawn_app().await;

	Mock::given(path("/email"))
		.and(method("POST"))
		.respond_with(ResponseTemplate::new(200))
		.mount(&app.email_server)
		.await;

	app.post_subscriptions("name=Andre%20Heber&email=andre.heber%40gmx.net".into()).await;
	let email_request = &app.email_server.received_requests().await.unwrap()[0];
	let confirmation_links = app.get_confirmation_links(email_request);

	let mut confirmation_link = Url::parse(&confirmation_links.html).unwrap();

	assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
	confirmation_link.set_port(Some(app.port)).unwrap();

	let _response = reqwest::get(confirmation_link)
		.await
		.unwrap()
		.error_for_status()
		.unwrap();

	let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
		.fetch_one(&app.connection_pool)
		.await
		.expect("Failed to fetch saved subscription");

		assert_eq!(saved.email, "andre.heber@gmx.net");
		assert_eq!(saved.name, "Andre Heber");
		assert_eq!(saved.status, "confirmed");
}
