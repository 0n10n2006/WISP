//! Free-model routing and the OpenRouter client against mocked servers.

use std::sync::Mutex;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{body_string_contains, header, method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wisp_core::gemini::prompt::{build_request, PromptContext};
use wisp_core::gemini::types::{AskRequest, Attachment, ContextKind, GenerateContentRequest};
use wisp_core::gemini::GeminiClient;
use wisp_core::openrouter::OpenRouterClient;
use wisp_core::retry::RetryPolicy;
use wisp_core::router::{self, Provider, QuotaBook, Route};
use wisp_core::ErrorCode;

static PERSONA: wisp_core::settings::Persona = wisp_core::settings::Persona {
    tone: wisp_core::settings::Tone::Concise,
    instructions: String::new(),
};

const GEMINI_KEY: &str = "test-key-123456";
const OR_KEY: &str = "sk-or-v1-secretsecretsecret0001";

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

fn body_for(req: &AskRequest, model: &str) -> GenerateContentRequest {
    build_request(
        req,
        &PromptContext {
            now_local: "2026-10-23T12:00:00Z",
            memories: &[],
            allowed_apps: &[],
            model,
            fast: true,
            persona: &PERSONA,
            desktop: None,
        },
    )
}

fn gemini(server: &MockServer) -> GeminiClient {
    let mut c = GeminiClient::with_base_url(server.uri());
    c.retry = RetryPolicy {
        base: Duration::from_millis(1),
        max_attempts: 1,
        ..RetryPolicy::default()
    };
    c
}

const DAILY_429: &str = r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"quota","details":[{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaId":"GenerateRequestsPerDayPerProjectPerModel-FreeTier"}]}]}}"#;

fn gemini_answer(text: &str) -> String {
    format!(
        "data: {{\"candidates\":[{{\"content\":{{\"role\":\"model\",\"parts\":[{{\"text\":\"{text}\"}}]}},\"finishReason\":\"STOP\"}}]}}\r\n\r\n"
    )
}

/// Ask through the router the way the app does.
async fn route_and_ask(
    gem: &GeminiClient,
    or: &OpenRouterClient,
    req: &AskRequest,
    routes: Vec<Route>,
    book: &Mutex<QuotaBook>,
) -> Result<(String, Route), wisp_core::WispError> {
    let cancel = CancellationToken::new();
    router::run_with_fallback(
        routes,
        book,
        || 1_790_000_000,
        |route| {
            let cancel = cancel.clone();
            async move {
                let mut r = req.clone();
                r.enable_tools &= route.caps.tools;
                r.web &= route.caps.search;
                let body = router::adapt_for(body_for(&r, &route.model), route.caps);
                let mut started = false;
                let res = match route.provider {
                    Provider::Gemini => {
                        gem.stream(GEMINI_KEY, &route.model, &body, &cancel, |_| started = true)
                            .await
                    }
                    Provider::OpenRouter => {
                        or.stream(OR_KEY, &route.model, &body, &cancel, |_| started = true)
                            .await
                    }
                };
                (res.map(|o| o.text), started)
            }
        },
    )
    .await
    .map(|a| (a.value, a.route))
    .map_err(|(e, _)| e)
}

#[tokio::test]
async fn a_used_up_gemini_model_hands_over_to_flash_lite_then_gemma() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/v1beta/models/gemini-2.5-flash:streamGenerateContent",
        ))
        .respond_with(ResponseTemplate::new(429).set_body_string(DAILY_429))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(
            "/v1beta/models/gemini-2.5-flash-lite:streamGenerateContent",
        ))
        .respond_with(ResponseTemplate::new(429).set_body_string(DAILY_429))
        .expect(1)
        .mount(&server)
        .await;
    // Gemma gets the system prompt folded into the question, and no tools.
    Mock::given(method("POST"))
        .and(path("/v1beta/models/gemma-3-27b-it:streamGenerateContent"))
        .and(body_string_contains("You are Wisp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(gemini_answer("from gemma")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let gem = gemini(&server);
    let or = OpenRouterClient::with_base_url(server.uri());
    let book = Mutex::new(QuotaBook::default());
    let prefs = router::Prefs {
        selected: "gemini-2.5-flash".into(),
        auto_fallback: true,
        ..Default::default()
    };
    let req = ask(
        "tell me a long story about a lighthouse keeper and the sea "
            .repeat(6)
            .as_str(),
    );
    let routes = router::candidates(
        router::classify(&req),
        &prefs,
        &[],
        &book.lock().unwrap(),
        0,
    );
    let (text, route) = route_and_ask(&gem, &or, &req, routes, &book).await.unwrap();
    assert_eq!(text, "from gemma");
    assert_eq!(route.model, "gemma-3-27b-it");

    // The next question skips the used-up models without asking them again.
    let routes = router::candidates(
        router::Task::Chat,
        &prefs,
        &[],
        &book.lock().unwrap(),
        1_790_000_000,
    );
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].model, "gemma-3-27b-it");
}

#[tokio::test]
async fn openrouter_answers_simple_jobs_and_streams() {
    let server = MockServer::start().await;
    let sse = concat!(
        ": OPENROUTER PROCESSING\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\" there\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n",
        "data: [DONE]\n\n"
    );
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", format!("Bearer {OR_KEY}").as_str()))
        .and(body_string_contains("\"stream\":true"))
        .and(body_string_contains("llama-3.3-70b-instruct:free"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse),
        )
        .expect(1)
        .mount(&server)
        .await;
    let gem = gemini(&server);
    let or = OpenRouterClient::with_base_url(server.uri());
    let book = Mutex::new(QuotaBook::default());
    let prefs = router::Prefs {
        selected: "gemini-2.5-flash".into(),
        auto_fallback: true,
        openrouter_model: "meta-llama/llama-3.3-70b-instruct:free".into(),
        simple_via_openrouter: true,
    };
    let mut req = ask("Summarise: the meeting moved to Friday.");
    req.context_kind = ContextKind::Clipboard;
    let routes = router::candidates(
        router::classify(&req),
        &prefs,
        &[],
        &book.lock().unwrap(),
        0,
    );
    assert_eq!(routes[0].provider, Provider::OpenRouter);
    let (text, route) = route_and_ask(&gem, &or, &req, routes, &book).await.unwrap();
    assert_eq!(text, "Hello there");
    assert_eq!(route.provider, Provider::OpenRouter);
}

#[tokio::test]
async fn openrouter_daily_limit_falls_back_to_gemini_without_leaking_the_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_string(format!(
            r#"{{"error":{{"code":429,"message":"Rate limit exceeded: free-models-per-day. key {OR_KEY}"}}}}"#
        )))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(
            r"^/v1beta/models/gemini-2\.5-flash-lite:streamGenerateContent$",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(gemini_answer("from lite")),
        )
        .mount(&server)
        .await;
    let gem = gemini(&server);
    let or = OpenRouterClient::with_base_url(server.uri());

    // Direct call: the error says what happened and never contains the key.
    let err = or
        .stream(
            OR_KEY,
            "a/b:free",
            &body_for(&ask("hi"), "x"),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::QuotaExhausted);
    assert!(!format!("{err:?}").contains(OR_KEY));

    let book = Mutex::new(QuotaBook::default());
    let prefs = router::Prefs {
        selected: "gemini-2.5-flash".into(),
        auto_fallback: true,
        openrouter_model: "a/b:free".into(),
        simple_via_openrouter: true,
    };
    let req = ask("what's a haiku?");
    let routes = router::candidates(
        router::classify(&req),
        &prefs,
        &[],
        &book.lock().unwrap(),
        0,
    );
    let (text, route) = route_and_ask(&gem, &or, &req, routes, &book).await.unwrap();
    assert_eq!(text, "from lite");
    assert_eq!(route.model, "gemini-2.5-flash-lite");
}

#[tokio::test]
async fn screenshots_are_never_sent_to_openrouter() {
    let server = MockServer::start().await;
    // Nothing at all may reach the server.
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let or = OpenRouterClient::with_base_url(server.uri());
    let mut req = ask("what is this?");
    req.attachments.push(Attachment {
        mime_type: "image/png".into(),
        data_base64: "AA".into(),
    });
    let err = or
        .stream(
            OR_KEY,
            "a/b:free",
            &body_for(&req, "x"),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
    let prefs = router::Prefs {
        selected: "gemini-2.5-flash".into(),
        auto_fallback: true,
        openrouter_model: "a/b:free".into(),
        simple_via_openrouter: true,
    };
    let routes = router::candidates(
        router::classify(&req),
        &prefs,
        &[],
        &QuotaBook::default(),
        0,
    );
    assert!(routes.iter().all(|r| r.provider == Provider::Gemini));
}

#[tokio::test]
async fn lists_free_models_and_checks_the_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/key"))
        .and(header("authorization", format!("Bearer {OR_KEY}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":{}}"#))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[
              {"id":"meta-llama/llama-3.3-70b-instruct:free","name":"Llama 3.3 70B (free)","context_length":131072,"pricing":{"prompt":"0","completion":"0"},"architecture":{"output_modalities":["text"]}},
              {"id":"mistralai/mistral-7b-instruct:free","name":"Mistral 7B (free)","context_length":32768,"pricing":{"prompt":"0","completion":"0"}},
              {"id":"openai/gpt-5","name":"Paid","context_length":1,"pricing":{"prompt":"0.00001","completion":"0.00003"}},
              {"id":"x/image-gen:free","name":"Images","context_length":1,"pricing":{"prompt":"0","completion":"0"},"architecture":{"output_modalities":["image"]}}
            ]}"#,
        ))
        .mount(&server)
        .await;
    let or = OpenRouterClient::with_base_url(server.uri());
    let models = or.list_free_models(OR_KEY).await.unwrap();
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "meta-llama/llama-3.3-70b-instruct:free",
            "mistralai/mistral-7b-instruct:free"
        ]
    );

    let bad = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/key"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(r#"{"error":{"code":401,"message":"No auth credentials found"}}"#),
        )
        .mount(&bad)
        .await;
    let err = OpenRouterClient::with_base_url(bad.uri())
        .list_free_models(OR_KEY)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidKey);
}
