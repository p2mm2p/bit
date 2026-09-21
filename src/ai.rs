//! AI 供给客户端：OpenAI 兼容的 `POST /chat/completions` 与 `GET /models`。
//!
//! 供给栈与端点形态见[决议 · AI 供给栈与 bit login 形态](https://github.com/p2mm2p/bit/issues/19)，
//! 各家差异以 [docs/research/ai-providers.md](../../docs/research/ai-providers.md) 为准；
//! 错误分类、超时与能力表的输入见[行为 · 提交消息生成细则（--gen）](https://github.com/p2mm2p/bit/issues/24)
//! 与[行为 · 配置存储与优先序](https://github.com/p2mm2p/bit/issues/25)。
//!
//! 本模块只做传输与结构化映射，不定义任何对外文案——文案由 `flow` 按
//! [命令面 · v0.2 增补](https://github.com/p2mm2p/bit/issues/27) 的冻结表渲染。
//! 同步、单轮、无重试；代理沿用 ureq 默认的环境变量行为（`ALL_PROXY` / `HTTPS_PROXY` /
//! `HTTP_PROXY`，含小写变体；`NO_PROXY` 生效），TLS 用 ureq 默认的 rustls + 内置根证书。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ureq::{Agent, Body};

/// 连接超时：10 秒（#24 冻结，v0.2 不可配）。
const TIMEOUT_CONNECT: Duration = Duration::from_secs(10);

/// 读取超时：90 秒（#24 冻结，v0.2 不可配），响应头与响应体各算一次。
const TIMEOUT_READ: Duration = Duration::from_secs(90);

/// 单条对话消息，只经构造函数产出。
#[derive(Clone, Debug, Serialize)]
pub struct Message {
    role: &'static str,
    content: String,
}

impl Message {
    /// `system` 消息（提示词）。
    pub fn system(content: &str) -> Message {
        Message {
            role: "system",
            content: content.to_string(),
        }
    }

    /// `user` 消息（送模型的输入）。
    pub fn user(content: &str) -> Message {
        Message {
            role: "user",
            content: content.to_string(),
        }
    }
}

/// 一次 `chat/completions` 请求；`stream` 恒为 `false`，`temperature` 与 `response_format` 按需带。
#[derive(Clone, Debug, Serialize)]
pub struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Clone, Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

impl ChatRequest {
    /// 组装请求：模型 + 消息，不带 `temperature` 与 `response_format`。
    pub fn new(model: &str, messages: Vec<Message>) -> ChatRequest {
        ChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
            temperature: None,
            response_format: None,
        }
    }

    /// 带上 `temperature`（描述翻译冻结为 `0`）。
    pub fn temperature(mut self, value: f32) -> ChatRequest {
        self.temperature = Some(value);
        self
    }

    /// 带上 `response_format: {"type":"json_object"}`；是否该带由 [`supports_json_object`] 判定。
    pub fn json_object(mut self) -> ChatRequest {
        self.response_format = Some(ResponseFormat {
            kind: "json_object",
        });
        self
    }
}

/// 一台已配置好的供给：端点、密钥与连接池。
pub struct Client {
    base_url: String,
    api_key: String,
    agent: Agent,
}

impl Client {
    /// 生产构造：超时 10 / 90 秒，代理与 TLS 都走 ureq 默认。
    pub fn new(base_url: &str, api_key: &str) -> Client {
        Self::with_timeouts(base_url, api_key, TIMEOUT_CONNECT, TIMEOUT_READ)
    }

    /// 超时可注入，供超时映射的单测用短值。
    fn with_timeouts(base_url: &str, api_key: &str, connect: Duration, read: Duration) -> Client {
        let config = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(connect))
            .timeout_recv_response(Some(read))
            .timeout_recv_body(Some(read))
            .build();
        Client {
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            agent: Agent::new_with_config(config),
        }
    }

    /// `chat/completions`：返回首个 choice 的 content。
    ///
    /// 非 2xx 一律 [`Error::Http`]（状态码 + 错误体里尽力抽出的短句）；2xx 但结构里
    /// 没有可用 content（缺 `choices`、缺 `message`、空串）为 [`Error::InvalidResponse`]。
    pub fn chat(&self, request: ChatRequest) -> Result<String, Error> {
        let response = self.post_chat(&request)?;
        response
            .content()
            .filter(|content| !content.is_empty())
            .map(str::to_owned)
            .ok_or(Error::InvalidResponse)
    }

    /// `GET /models`：按响应顺序返回模型 id。
    pub fn list_models(&self) -> Result<Vec<String>, Error> {
        let mut response = self
            .agent
            .get(endpoint(&self.base_url, "models"))
            .header("Authorization", &self.authorization())
            .call()
            .map_err(map_error)?;
        let body = read_response(&mut response)?;
        parse_models(&body)
    }

    /// 连通性验证（`bit login` 的「正在验证连通性…」）：一次最小 chat 调用，
    /// 拿到 2xx 的可用信封即通过；401 / 404 等状态由调用方按 [`Error::status`] 分流。
    ///
    /// 形状固定为一条 `ping` user 消息，不带 `temperature` / `response_format` / `max_tokens`——
    /// 参数面越小，各家兼容性越稳（与 #23 不引 `max_tokens` 同一条取舍）。
    pub fn verify(&self, model: &str) -> Result<(), Error> {
        self.post_chat(&verify_request(model)).map(|_| ())
    }

    fn post_chat(&self, request: &ChatRequest) -> Result<ChatEnvelope, Error> {
        let body = serde_json::to_vec(request).expect("ChatRequest 全字段可序列化");
        let mut response = self
            .agent
            .post(endpoint(&self.base_url, "chat/completions"))
            .header("Content-Type", "application/json")
            .header("Authorization", &self.authorization())
            .send(body.as_slice())
            .map_err(map_error)?;
        let body = read_response(&mut response)?;
        parse_chat(&body)
    }

    fn authorization(&self) -> String {
        format!("Bearer {}", self.api_key)
    }
}

/// `response_format: json_object` 的静态能力表（#24，键取 #25 的九种 provider）。
///
/// 智谱与 OpenRouter 的文档不承诺 `json_object`，`custom` 无从判断——一律不带，
/// 走提示词里的 JSON 约束兜底。
pub fn supports_json_object(provider: &str) -> bool {
    matches!(
        provider,
        "openai" | "deepseek" | "kimi" | "dashscope" | "siliconflow" | "ollama"
    )
}

/// 调用失败，按 #23 / #24 的失败分类结构化；对外文案由 flow 渲染。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// 没拿到 HTTP 响应：连接、DNS、TLS、代理、超时、响应体读取中断。
    Transport { reason: String },
    /// 非 2xx 响应；`message` 是从错误体里尽力抽出的短句（各家形状不一，可为 `None`）。
    Http {
        status: u16,
        message: Option<String>,
    },
    /// 2xx 但响应体不是可用结构。
    InvalidResponse,
}

/// 冻结表里的失败大类（#23 / #24 / #27）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 401 / 403：认证。
    Auth,
    /// 429：限流。
    RateLimited,
    /// 其余 4xx：模型名或参数（含 404）。
    ModelOrRequest,
    /// 5xx：服务端。
    Server,
    /// 连不上或超时。
    Network,
    /// 响应不可解析 / content 不可用。
    InvalidResponse,
}

impl Error {
    /// 冻结表用的大类。
    pub fn kind(&self) -> Kind {
        match self {
            Error::Transport { .. } => Kind::Network,
            Error::InvalidResponse => Kind::InvalidResponse,
            Error::Http { status, .. } => match *status {
                401 | 403 => Kind::Auth,
                429 => Kind::RateLimited,
                500..=599 => Kind::Server,
                _ => Kind::ModelOrRequest,
            },
        }
    }

    /// HTTP 状态码；仅 [`Error::Http`] 有。
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Http { status, .. } => Some(*status),
            _ => None,
        }
    }
}

/// 拼端点：`base_url` 的尾斜杠可省，路径总是一段。
fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{path}", base_url.trim_end_matches('/'))
}

/// 连通性验证的最小形状：一条 `ping`，不加任何可选参数。
fn verify_request(model: &str) -> ChatRequest {
    ChatRequest::new(model, vec![Message::user("ping")])
}

/// 读响应体并按状态分流：先读完（错误体里的短句要取），再判 2xx。
fn read_response(response: &mut ureq::http::Response<Body>) -> Result<String, Error> {
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(map_body_error)?;
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(Error::Http {
            status,
            message: error_message(&body),
        })
    }
}

/// 从各家形状不一的错误体里尽力抽一句可读消息：`/error/message`、`/message`、字符串 `/error`。
fn error_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let message = ["/error/message", "/message", "/error"]
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))?;
    let message = message.trim().chars().take(200).collect::<String>();
    (!message.is_empty()).then_some(message)
}

/// ureq 的失败 → 结构化失败：状态码兜底成 [`Error::Http`]，其余（连接、DNS、TLS、超时）
/// 都算传输层；`flow` 统一按「网络错误或超时」渲染。
fn map_error(error: ureq::Error) -> Error {
    match error {
        ureq::Error::StatusCode(status) => Error::Http {
            status,
            message: None,
        },
        other => Error::Transport {
            reason: other.to_string(),
        },
    }
}

/// 读体失败也算传输层，唯超限视为响应不可用（服务端给了荒唐的大响应）。
fn map_body_error(error: ureq::Error) -> Error {
    match error {
        ureq::Error::BodyExceedsLimit(_) => Error::InvalidResponse,
        other => map_error(other),
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct ChatEnvelope {
    choices: Vec<Choice>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

impl ChatEnvelope {
    fn content(&self) -> Option<&str> {
        self.choices.first()?.message.content.as_deref()
    }
}

fn parse_chat(body: &str) -> Result<ChatEnvelope, Error> {
    serde_json::from_str(body).map_err(|_| Error::InvalidResponse)
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

fn parse_models(body: &str) -> Result<Vec<String>, Error> {
    let list: ModelList = serde_json::from_str(body).map_err(|_| Error::InvalidResponse)?;
    Ok(list.data.into_iter().map(|entry| entry.id).collect())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread::{self, JoinHandle};
    use std::time::Instant;

    use serde_json::{Value, json};

    use super::*;

    // ---- 本地 stub：回一次写死的响应，并把收到的请求原文交回 ----

    fn serve(response: String) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口");
        let port = listener.local_addr().expect("读本地地址").port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("接受连接");
            let request = read_request(&mut stream);
            stream.write_all(response.as_bytes()).expect("写响应");
            request
        });
        (format!("http://127.0.0.1:{port}"), server)
    }

    fn http_response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn read_request(stream: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("读请求");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(head) = bytes.windows(4).position(|window| window == b"\r\n\r\n")
                && bytes.len() >= head + 4 + content_length(&bytes[..head])
            {
                break;
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn content_length(head: &[u8]) -> usize {
        String::from_utf8_lossy(head)
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }

    fn request_line(request: &str) -> &str {
        request.lines().next().unwrap_or_default()
    }

    fn header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
        request
            .lines()
            .take_while(|line| !line.is_empty())
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case(name).then(|| value.trim())
            })
    }

    fn request_body(request: &str) -> &str {
        request
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or("")
    }

    fn chat_body(content: &str) -> String {
        json!({"choices": [{"message": {"role": "assistant", "content": content}}]}).to_string()
    }

    // ---- 纯函数 ----

    #[test]
    fn endpoint_join_drops_the_trailing_slash() {
        assert_eq!(
            endpoint("https://api.openai.com/v1", "chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            endpoint("https://open.bigmodel.cn/api/paas/v4/", "models"),
            "https://open.bigmodel.cn/api/paas/v4/models"
        );
    }

    #[test]
    fn chat_request_shape_is_minimal_until_asked() {
        let request = ChatRequest::new(
            "kimi-k3",
            vec![Message::system("提示词"), Message::user("登录页")],
        );
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "model": "kimi-k3",
                "messages": [
                    {"role": "system", "content": "提示词"},
                    {"role": "user", "content": "登录页"},
                ],
                "stream": false,
            })
        );

        let request = request.temperature(0.0).json_object();
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "model": "kimi-k3",
                "messages": [
                    {"role": "system", "content": "提示词"},
                    {"role": "user", "content": "登录页"},
                ],
                "stream": false,
                "temperature": 0.0,
                "response_format": {"type": "json_object"},
            })
        );
    }

    #[test]
    fn capability_table_matches_the_decision() {
        for provider in [
            "openai",
            "deepseek",
            "kimi",
            "dashscope",
            "siliconflow",
            "ollama",
        ] {
            assert!(
                supports_json_object(provider),
                "{provider} 应带 json_object"
            );
        }
        for provider in ["zhipu", "openrouter", "custom", "unknown", ""] {
            assert!(
                !supports_json_object(provider),
                "{provider} 不应带 json_object"
            );
        }
    }

    #[test]
    fn error_message_handles_provider_shapes() {
        assert_eq!(
            error_message(r#"{"error":{"message":"Invalid API key"}}"#).as_deref(),
            Some("Invalid API key")
        );
        assert_eq!(
            error_message(r#"{"message":"oops"}"#).as_deref(),
            Some("oops")
        );
        assert_eq!(
            error_message(r#"{"error":"legacy shape"}"#).as_deref(),
            Some("legacy shape")
        );
        assert_eq!(error_message(r#"{"error":{"code":"1210"}}"#), None);
        assert_eq!(error_message(r#"{"error":{"message":"   "}}"#), None);
        assert_eq!(error_message("plain text"), None);
        assert_eq!(error_message(""), None);
    }

    #[test]
    fn error_message_truncates_by_chars() {
        let body = json!({"error": {"message": "错".repeat(500)}}).to_string();
        let message = error_message(&body).unwrap();
        assert_eq!(message.chars().count(), 200);
    }

    #[test]
    fn kind_covers_the_frozen_classes() {
        let http = |status| Error::Http {
            status,
            message: None,
        };
        assert_eq!(http(401).kind(), Kind::Auth);
        assert_eq!(http(403).kind(), Kind::Auth);
        assert_eq!(http(404).kind(), Kind::ModelOrRequest);
        assert_eq!(http(400).kind(), Kind::ModelOrRequest);
        assert_eq!(http(422).kind(), Kind::ModelOrRequest);
        assert_eq!(http(429).kind(), Kind::RateLimited);
        assert_eq!(http(500).kind(), Kind::Server);
        assert_eq!(http(503).kind(), Kind::Server);
        assert_eq!(
            Error::Transport {
                reason: "x".to_string()
            }
            .kind(),
            Kind::Network
        );
        assert_eq!(Error::InvalidResponse.kind(), Kind::InvalidResponse);
        assert_eq!(http(404).status(), Some(404));
        assert_eq!(Error::InvalidResponse.status(), None);
        assert_eq!(
            Error::Transport {
                reason: "x".to_string()
            }
            .status(),
            None
        );
    }

    #[test]
    fn parse_models_reads_ids_in_order() {
        let models = parse_models(r#"{"object":"list","data":[{"id":"b"},{"id":"a"}]}"#).unwrap();
        assert_eq!(models, ["b", "a"]);
        assert!(
            parse_models(r#"{"object":"list","data":[]}"#)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            parse_models(r#"{"data":[{"object":"model"}]}"#),
            Err(Error::InvalidResponse)
        );
        assert_eq!(parse_models("not json"), Err(Error::InvalidResponse));
    }

    #[test]
    fn parse_chat_requires_the_choices_envelope() {
        assert_eq!(parse_chat("not json"), Err(Error::InvalidResponse));
        assert_eq!(parse_chat("{}"), Err(Error::InvalidResponse));
        assert_eq!(
            parse_chat(r#"{"choices":[{}]}"#),
            Err(Error::InvalidResponse)
        );
        assert_eq!(parse_chat(r#"{"choices":[]}"#).unwrap().content(), None);
        assert_eq!(
            parse_chat(r#"{"choices":[{"message":{"content":null}}]}"#)
                .unwrap()
                .content(),
            None
        );
        assert_eq!(
            parse_chat(r#"{"choices":[{"message":{"content":"ok"}}]}"#)
                .unwrap()
                .content(),
            Some("ok")
        );
    }

    // ---- 走本地 stub 的网络路径 ----

    #[test]
    fn chat_posts_openai_shape_and_returns_content() {
        let (base, server) = serve(http_response("200 OK", &chat_body("add-oauth-login")));
        let client = Client::new(&base, "sk-test");
        let request = ChatRequest::new(
            "kimi-k3",
            vec![Message::system("提示词"), Message::user("添加 OAuth 登录")],
        )
        .temperature(0.0);
        assert_eq!(client.chat(request).unwrap(), "add-oauth-login");

        let sent = server.join().unwrap();
        assert_eq!(request_line(&sent), "POST /chat/completions HTTP/1.1");
        assert_eq!(header(&sent, "authorization"), Some("Bearer sk-test"));
        assert_eq!(header(&sent, "content-type"), Some("application/json"));
        assert_eq!(
            serde_json::from_str::<Value>(request_body(&sent)).unwrap(),
            json!({
                "model": "kimi-k3",
                "messages": [
                    {"role": "system", "content": "提示词"},
                    {"role": "user", "content": "添加 OAuth 登录"},
                ],
                "stream": false,
                "temperature": 0.0,
            })
        );
    }

    #[test]
    fn list_models_joins_the_endpoint_and_reads_the_list() {
        let (base, server) = serve(http_response(
            "200 OK",
            r#"{"data":[{"id":"deepseek-chat"},{"id":"deepseek-reasoner"}]}"#,
        ));
        // 尾斜杠留给 endpoint 归一，路径里不应出现 `//`。
        let client = Client::new(&format!("{base}/"), "sk-test");
        assert_eq!(
            client.list_models().unwrap(),
            ["deepseek-chat", "deepseek-reasoner"]
        );

        let sent = server.join().unwrap();
        assert_eq!(request_line(&sent), "GET /models HTTP/1.1");
        assert_eq!(header(&sent, "authorization"), Some("Bearer sk-test"));
    }

    #[test]
    fn http_error_keeps_status_and_error_message() {
        let (base, server) = serve(http_response(
            "401 Unauthorized",
            r#"{"error":{"message":"Invalid API key"}}"#,
        ));
        let client = Client::new(&base, "sk-bad");
        let error = client.list_models().unwrap_err();
        assert_eq!(error.kind(), Kind::Auth);
        assert_eq!(error.status(), Some(401));
        match error {
            Error::Http { message, .. } => {
                assert_eq!(message.as_deref(), Some("Invalid API key"));
            }
            other => panic!("应为 Http，实际是 {other:?}"),
        }
        server.join().unwrap();
    }

    #[test]
    fn verify_sends_the_minimal_request() {
        let (base, server) = serve(http_response(
            "200 OK",
            r#"{"choices":[{"message":{"role":"assistant","content":"OK"}}]}"#,
        ));
        let client = Client::new(&base, "sk-test");
        client.verify("GLM-5.3").unwrap();

        let sent = server.join().unwrap();
        assert_eq!(request_line(&sent), "POST /chat/completions HTTP/1.1");
        assert_eq!(
            serde_json::from_str::<Value>(request_body(&sent)).unwrap(),
            json!({
                "model": "GLM-5.3",
                "messages": [{"role": "user", "content": "ping"}],
                "stream": false,
            })
        );
    }

    #[test]
    fn verify_accepts_an_envelope_without_content_but_chat_does_not() {
        let (base, server) = serve(http_response(
            "200 OK",
            r#"{"choices":[{"message":{"content":null}}]}"#,
        ));
        Client::new(&base, "sk-test").verify("kimi-k3").unwrap();
        server.join().unwrap();

        let (base, server) = serve(http_response(
            "200 OK",
            r#"{"choices":[{"message":{"content":""}}]}"#,
        ));
        let client = Client::new(&base, "sk-test");
        let request = ChatRequest::new("kimi-k3", vec![Message::user("hi")]);
        assert_eq!(client.chat(request), Err(Error::InvalidResponse));
        server.join().unwrap();
    }

    #[test]
    fn malformed_success_body_is_invalid_response() {
        let (base, server) = serve(http_response("200 OK", "<html>proxy</html>"));
        let client = Client::new(&base, "sk-test");
        let request = ChatRequest::new("kimi-k3", vec![Message::user("hi")]);
        assert_eq!(client.chat(request), Err(Error::InvalidResponse));
        server.join().unwrap();
    }

    #[test]
    fn refused_connection_is_transport() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口");
        let port = listener.local_addr().expect("读本地地址").port();
        drop(listener);
        let client = Client::new(&format!("http://127.0.0.1:{port}"), "sk-test");
        assert!(matches!(client.list_models(), Err(Error::Transport { .. })));
    }

    #[test]
    fn read_timeout_is_transport() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口");
        let port = listener.local_addr().expect("读本地地址").port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("接受连接");
            thread::sleep(Duration::from_millis(500));
            drop(stream);
        });
        let client = Client::with_timeouts(
            &format!("http://127.0.0.1:{port}"),
            "sk-test",
            Duration::from_secs(1),
            Duration::from_millis(100),
        );
        let started = Instant::now();
        let error = client.list_models().unwrap_err();
        assert!(matches!(error, Error::Transport { .. }), "实际是 {error:?}");
        assert!(
            started.elapsed() < Duration::from_millis(450),
            "不应等满 stub 的 500ms"
        );
        server.join().unwrap();
    }
}
