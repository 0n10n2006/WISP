//! Gemini client against a mocked HTTP server — no live key needed.

use std::time::Duration;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wisp_core::gemini::prompt::{build_request, PromptContext};
use wisp_core::gemini::types::{AskRequest, ContextKind};
use wisp_core::gemini::GeminiClient;
use wisp_core::retry::RetryPolicy;
use wisp_core::ErrorCode;

static PERSONA: wisp_core::settings::Persona = wisp_core::settings::Persona {
    tone: wisp_core::settings::Tone::Concise,
    instructions: String::new(),
};

const KEY: &str = "test-key-123456";

fn ask(prompt: &str) -> AskRequest {
    AskRequest {
        request_id: "r".into(),
        prompt: prompt.into(),
        history: vec![],
        attachments: vec![],
        context_kind: ContextKind::Text,
        use_memory: false,
        enable_tools: false,
        think: false,
        web: false,
    }
}

fn ctx() -> PromptContext<'static> {
    PromptContext {
        now_local: "2026-10-21T12:00:00Z",
        memories: &[],
        allowed_apps: &[],
        model: "gemini-2.5-flash",
        fast: true,
        persona: &PERSONA,
        desktop: None,
    }
}

fn client(server: &MockServer) -> GeminiClient {
    let mut c = GeminiClient::with_base_url(server.uri());
    c.retry = RetryPolicy {
        base: Duration::from_millis(1),
        ..RetryPolicy::default()
    };
    c
}

fn sse(chunks: &[&str]) -> String {
    chunks
        .iter()
        .map(|c| format!("data: {c}\r\n\r\n"))
        .collect()
}

#[tokio::test]
async fn streams_text_deltas() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1beta/models/gemini-test:streamGenerateContent"))
        .and(query_param("alt", "sse"))
        .and(header("x-goog-api-key", KEY))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(sse(&[
            r#"{"candidates":[{"content":{"role":"model","parts":[{"text":"Hel"}]}}]}"#,
            r#"{"candidates":[{"content":{"role":"model","parts":[{"thought":true,"text":"secret plan"},{"text":"lo!"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":2,"totalTokenCount":5}}"#,
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let mut deltas = Vec::new();
    let out = client(&server)
        .stream(
            KEY,
            "gemini-test",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |d| deltas.push(d.to_string()),
        )
        .await
        .unwrap();
    assert_eq!(deltas, vec!["Hel", "lo!"]);
    assert_eq!(out.text, "Hello!");
    assert_eq!(out.usage.unwrap().total_token_count, 5);
}

#[tokio::test]
async fn retries_server_errors_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503).set_body_string("{}"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse(&[
            r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]}}]}"#,
        ])))
        .mount(&server)
        .await;
    let out = client(&server)
        .stream(
            KEY,
            "m",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(out.text, "ok");
}

#[tokio::test]
async fn quota_exhaustion_is_not_retried() {
    let server = MockServer::start().await;
    let body = r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"quota","details":[{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaId":"GenerateRequestsPerDayPerProjectPerModel-FreeTier"}]}]}}"#;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    let err = client(&server)
        .stream(
            KEY,
            "m",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::QuotaExhausted);
}

#[tokio::test]
async fn retries_are_bounded() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("{}"))
        .expect(3)
        .mount(&server)
        .await;
    let err = client(&server)
        .stream(
            KEY,
            "m",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ServerError);
}

#[tokio::test]
async fn malformed_stream_is_reported() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("data: {not json\n\n"))
        .mount(&server)
        .await;
    let err = client(&server)
        .stream(
            KEY,
            "m",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::MalformedResponse);
}

#[tokio::test]
async fn blocked_prompt_is_reported() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(sse(&[r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#])),
        )
        .mount(&server)
        .await;
    let err = client(&server)
        .stream(
            KEY,
            "m",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Blocked);
}

#[tokio::test]
async fn cancellation_stops_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(5))
                .set_body_string("data: {}\n\n"),
        )
        .mount(&server)
        .await;
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        c2.cancel();
    });
    let started = std::time::Instant::now();
    let err = client(&server)
        .stream(
            KEY,
            "m",
            &build_request(&ask("hi"), &ctx()),
            &cancel,
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn first_byte_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(3)))
        .mount(&server)
        .await;
    let mut c = client(&server);
    c.first_byte_timeout = Duration::from_millis(100);
    c.retry = RetryPolicy::none();
    let err = c
        .stream(
            KEY,
            "m",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Timeout);
}

#[tokio::test]
async fn missing_key_fails_fast() {
    let err = GeminiClient::with_base_url("http://127.0.0.1:9")
        .stream(
            "",
            "m",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::MissingKey);
}

#[tokio::test]
async fn offline_is_reported() {
    let mut c = GeminiClient::with_base_url("http://127.0.0.1:9");
    c.retry = RetryPolicy::none();
    let err = c
        .stream(
            KEY,
            "m",
            &build_request(&ask("hi"), &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Offline);
}

#[tokio::test]
async fn lists_and_filters_models() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(header("x-goog-api-key", KEY))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"models":[
                {"name":"models/gemini-2.5-flash","displayName":"Gemini 2.5 Flash","supportedGenerationMethods":["generateContent","countTokens"],"inputTokenLimit":1048576},
                {"name":"models/text-embedding-004","supportedGenerationMethods":["embedContent"]},
                {"name":"models/gemini-embedding-001","supportedGenerationMethods":["generateContent"]},
                {"name":"models/gemini-2.5-flash-lite","supportedGenerationMethods":["generateContent"]}
            ]}"#,
        ))
        .mount(&server)
        .await;
    let report = client(&server)
        .test_connection(KEY, "gemini-2.5-flash")
        .await
        .unwrap();
    let ids: Vec<_> = report.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["gemini-2.5-flash", "gemini-2.5-flash-lite"]);
    assert!(report.model_available);
    assert_eq!(report.models[1].display_name, "gemini-2.5-flash-lite");
}

#[tokio::test]
async fn invalid_key_on_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(400).set_body_string(
            r#"{"error":{"code":400,"message":"API key not valid.","status":"INVALID_ARGUMENT","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"API_KEY_INVALID"}]}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let err = client(&server).list_models(KEY).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidKey);
    assert!(!err.message.contains(KEY));
}

#[tokio::test]
async fn function_calls_are_collected() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1beta/models/m:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"open_url","args":{"url":"https://example.com"}}}]},"finishReason":"STOP"}]}"#,
        ))
        .mount(&server)
        .await;
    let out = client(&server)
        .generate(
            KEY,
            "m",
            &build_request(&ask("open example"), &ctx()),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(out.function_calls.len(), 1);
    assert_eq!(out.function_calls[0].name, "open_url");
    assert_eq!(out.model_parts.len(), 1);
}

#[tokio::test]
async fn function_call_round_trip_echoes_call_and_result() {
    use wisp_core::gemini::prompt::with_function_results;
    use wisp_core::gemini::types::FunctionResponse;
    use wisp_core::tools::validate_call;

    let server = MockServer::start().await;
    // Round 2: the request must contain our functionResponse.
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_string_contains("functionResponse"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse(&[
            r#"{"candidates":[{"content":{"parts":[{"text":"Done — I opened it."}]},"finishReason":"STOP"}]}"#,
        ])))
        .expect(1)
        .mount(&server)
        .await;
    // Round 1: the model proposes an action (with a thought signature to echo back).
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse(&[
            r#"{"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"open_url","args":{"url":"https://example.com"}},"thoughtSignature":"sig123"}]},"finishReason":"STOP"}]}"#,
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let mut req = ask("open example.com");
    req.enable_tools = true;
    let body = build_request(&req, &ctx());
    assert!(body.tools.is_some());
    let first = c
        .stream(KEY, "m", &body, &CancellationToken::new(), |_| {})
        .await
        .unwrap();
    assert_eq!(first.function_calls.len(), 1);
    let call = &first.function_calls[0];
    let action = validate_call(&call.name, &call.args, &[], chrono::Utc::now()).unwrap();
    assert!(action.requires_confirmation());

    let next = with_function_results(
        body,
        first.model_parts.clone(),
        vec![FunctionResponse {
            name: call.name.clone(),
            response: serde_json::json!({"ok": true, "result": "Opened."}),
        }],
    );
    let json = serde_json::to_string(&next).unwrap();
    assert!(json.contains("\"thoughtSignature\":\"sig123\""));
    let second = c
        .stream(KEY, "m", &next, &CancellationToken::new(), |_| {})
        .await
        .unwrap();
    assert_eq!(second.text, "Done — I opened it.");
}

#[tokio::test]
async fn update_check_reads_latest_release() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/o/r/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"tag_name":"v0.3.0","html_url":"https://github.com/o/r/releases/tag/v0.3.0","draft":false}"#,
        ))
        .mount(&server)
        .await;
    let info = wisp_core::updates::check(&server.uri(), "o/r", "0.2.0")
        .await
        .unwrap();
    assert!(info.available);
    assert_eq!(info.latest.as_deref(), Some("0.3.0"));
    // A release link anywhere else is never offered.
    let other = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/o/r/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"tag_name":"v0.3.0","html_url":"https://evil.example/o/r/releases/tag/v0.3.0","draft":false}"#,
        ))
        .mount(&other)
        .await;
    let phish = wisp_core::updates::check(&other.uri(), "o/r", "0.2.0")
        .await
        .unwrap();
    assert!(!phish.available && phish.url.is_none());
    let same = wisp_core::updates::check(&server.uri(), "o/r", "0.3.0")
        .await
        .unwrap();
    assert!(!same.available);
}

#[tokio::test]
async fn update_check_without_releases() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let info = wisp_core::updates::check(&server.uri(), "o/r", "0.2.0")
        .await
        .unwrap();
    assert!(!info.available);
    assert!(info.latest.is_none());
}

#[tokio::test]
async fn grounded_answers_collect_sources() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_string_contains("google_search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse(&[
            r#"{"candidates":[{"content":{"parts":[{"text":"It is sunny."}]}}]}"#,
            r#"{"candidates":[{"content":{"parts":[{"text":""}]},"finishReason":"STOP","groundingMetadata":{"groundingChunks":[{"web":{"uri":"https://weather.example/leeds","title":"Weather Example"}},{"web":{"uri":"https://weather.example/leeds","title":"dup"}},{"web":{"uri":"javascript:alert(1)","title":"bad"}}]}}]}"#,
        ])))
        .mount(&server)
        .await;
    let mut req = ask("weather in leeds");
    req.web = true;
    let out = client(&server)
        .stream(
            KEY,
            "m",
            &build_request(&req, &ctx()),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(out.text, "It is sunny.");
    assert_eq!(out.sources.len(), 1);
    assert_eq!(out.sources[0].title, "Weather Example");
}

#[tokio::test]
async fn weather_search_and_forecast() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/search"))
        .and(query_param("name", "Leeds"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"results":[{"name":"Leeds","latitude":53.8,"longitude":-1.55,"country":"United Kingdom","admin1":"England"}]}"#,
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/forecast"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"current":{"temperature_2m":12.5,"weather_code":61,"wind_speed_10m":18.0,"is_day":1},
               "daily":{"time":["2026-10-21","2026-10-22"],"temperature_2m_max":[14,15],"temperature_2m_min":[8,9],"weather_code":[61,3]}}"#,
        ))
        .mount(&server)
        .await;
    let places = wisp_core::net::search_places(&server.uri(), "Leeds")
        .await
        .unwrap();
    assert_eq!(places[0].name, "Leeds, England, United Kingdom");
    let w = wisp_core::net::forecast(&server.uri(), &places[0])
        .await
        .unwrap();
    assert_eq!(w.label, "Rain");
    assert_eq!(w.days.len(), 2);
    assert_eq!(w.days[1].label, "Overcast");
    assert!(wisp_core::net::search_places(&server.uri(), "x")
        .await
        .is_err());
}

#[tokio::test]
async fn speed_test_measures_download() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; 2_000_000]))
        .mount(&server)
        .await;
    let r = wisp_core::net::speed_test(&format!("{}/down", server.uri()))
        .await
        .unwrap();
    assert_eq!(r.bytes, 2_000_000);
    assert!(r.mbps > 0.0);
}

#[tokio::test]
async fn never_follows_redirects_with_the_key() {
    let server = MockServer::start().await;
    let elsewhere = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/steal", elsewhere.uri()).as_str()),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"models\":[]}"))
        .expect(0)
        .mount(&elsewhere)
        .await;
    assert!(client(&server).list_models(KEY).await.is_err());
    elsewhere.verify().await;
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake image bytes";

#[tokio::test]
async fn lens_follows_its_redirect_to_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/upload"))
        .respond_with(
            ResponseTemplate::new(303)
                .insert_header("location", "https://lens.google.com/search?p=XYZ&ep=gsbubb"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let url = wisp_core::net::lens_upload(&server.uri(), PNG)
        .await
        .unwrap();
    assert_eq!(url, "https://lens.google.com/search?p=XYZ&ep=gsbubb");
    let req = &server.received_requests().await.unwrap()[0];
    let ct = req.headers.get("content-type").unwrap().to_str().unwrap();
    assert!(ct.starts_with("multipart/form-data; boundary="));
    let body = String::from_utf8_lossy(&req.body);
    assert!(body.contains("name=\"encoded_image\""));
}

#[tokio::test]
async fn lens_reads_results_from_a_page_and_rejects_other_hosts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/upload"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<html><meta content="0;url=https://lens.google.com/search?p=Q1&amp;ep=x"></html>"#,
        ))
        .mount(&server)
        .await;
    assert_eq!(
        wisp_core::net::lens_upload(&server.uri(), PNG)
            .await
            .unwrap(),
        "https://lens.google.com/search?p=Q1&ep=x"
    );

    let other = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", "https://evil.example/phish"),
        )
        .mount(&other)
        .await;
    assert!(wisp_core::net::lens_upload(&other.uri(), PNG)
        .await
        .is_err());
    assert!(wisp_core::net::lens_upload(&other.uri(), b"not a png")
        .await
        .is_err());
}
