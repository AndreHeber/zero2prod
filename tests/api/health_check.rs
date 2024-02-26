use crate::helpers::spawn_app;

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;
    let health_check_endpoint = format!("{}/health_check", app.address);
    let client = reqwest::Client::new();
    let response = client
        .get(health_check_endpoint)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}