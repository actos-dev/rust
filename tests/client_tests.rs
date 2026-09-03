use actos::{Actos, ActosBuilder, Page, paginate_stream};
use futures_util::StreamExt;
use reqwest::Method;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn test_builder_from_env() {
    let builder = ActosBuilder::from_env();
    let client = builder.build().expect("client from env");
    assert!(client.base_url().as_str().contains("http"));
}

#[tokio::test]
async fn test_client_request_escape_hatch_with_server() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/custom/resource"))
        .and(header("authorization", "Bearer secret_123"))
        .and(header("user-agent", "actos-rust/0.1.0"))
        .and(header("accept", "application/json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"custom": "data"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("secret_123")
        .build()
        .expect("valid client");

    let req_builder = client.request(Method::GET, "/custom/resource");
    let res = req_builder.send().await.expect("send succeeds");
    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.expect("json deserializes");
    assert_eq!(body["custom"], "data");
}

#[tokio::test]
async fn test_paginate_stream_full_lifecycle() {
    let fetcher = |cursor: Option<String>| async move {
        match cursor.as_deref() {
            None => Ok(Page::new(
                vec!["item1".to_string(), "item2".to_string()],
                Some("page2".to_string()),
            )),
            Some("page2") => Ok(Page::new(vec!["item3".to_string()], None)),
            Some(_) => Err(actos::Error::Config("unknown cursor".to_string())),
        }
    };

    let stream = paginate_stream(fetcher);
    let results: Vec<actos::Result<String>> = stream.collect().await;

    assert_eq!(results.len(), 3);
    let items: Vec<String> = results
        .into_iter()
        .map(|r| r.expect("valid item"))
        .collect();
    assert_eq!(items, vec!["item1", "item2", "item3"]);
}
