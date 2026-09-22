//! `bit branch` 描述翻译的 pty e2e（[实现 · 描述翻译接入 bit branch](https://github.com/p2mm2p/bit/issues/31) 的验收面）。
//!
//! 行为对照 [行为 · 描述翻译细则](https://github.com/p2mm2p/bit/issues/23)，
//! 文案逐字对照 [命令面 · v0.2 增补](https://github.com/p2mm2p/bit/issues/27) 的 B4、B6、B10、B12–B19；
//! 网络面全部打给 `tests/common` 的本地 stub 供给（真 HTTP、不联网、不碰真 key）。

mod common;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{AiReply, AiStub, Fixture, Pty, chat_body};

/// 一份可用的自定义家配置：四个键指到 stub。
fn config_toml(base_url: &str) -> String {
    format!(
        "provider = \"custom\"\n\
         base_url = \"{base_url}\"\n\
         model = \"stub-model\"\n\
         api_key = \"sk-test-123456\"\n"
    )
}

/// 正向：中文触发翻译 → B12 进度 → B10 回显 → 确认后真建出英文名的分支。
#[test]
fn translation_success_creates_and_switches_branch() {
    if common::ensure_console("translation_success_creates_and_switches_branch") {
        return;
    }
    let fixture = Fixture::new();
    let stub = AiStub::start(|request| match request.path.as_str() {
        "/chat/completions" => AiReply::ok(chat_body("add-oauth-login")),
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\r"); // 默认停在第一项：feature
    pty.expect_screen("可直接写中文，自动翻译为英文"); // B4

    pty.send("添加 OAuth 登录");
    pty.send("\r");
    pty.expect_screen("正在翻译描述…"); // B12
    pty.expect_screen_flat(r#"由 "添加 OAuth 登录" 翻译为 "add-oauth-login""#); // B10
    pty.expect_screen("feature/add-oauth-login");
    pty.send("\r");

    pty.expect_screen("Switched to a new branch");
    assert_eq!(pty.exit_code(), 0);
    assert_eq!(
        fixture.git_ok(&["branch", "--show-current"]),
        "feature/add-oauth-login"
    );
}

/// 401：B13 报错 → 回名称输入预填原文（直接回车再翻一次）→ 第二次成功。
#[test]
fn auth_failure_returns_to_input_with_the_original_text_prefilled() {
    if common::ensure_console("auth_failure_returns_to_input_with_the_original_text_prefilled") {
        return;
    }
    let fixture = Fixture::new();
    let chats = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&chats);
    let stub = AiStub::start(move |request| match request.path.as_str() {
        "/chat/completions" => {
            if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                AiReply::status(401, r#"{"error":{"message":"bad key"}}"#)
            } else {
                AiReply::ok(chat_body("add-login"))
            }
        }
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\r");
    pty.expect_screen("分支描述");
    pty.send("添加登录");
    pty.send("\r");

    pty.expect_screen_flat("翻译失败：认证失败（401），请先跑 bit login 检查密钥。"); // B13
    pty.send("\r"); // 预填的该是原文：直接回车会拿它再翻一次
    pty.expect_screen_flat(r#"由 "添加登录" 翻译为 "add-login""#);
    pty.send("\r");

    pty.expect_screen("Switched to a new branch");
    assert_eq!(pty.exit_code(), 0);
    assert_eq!(
        chats.load(Ordering::SeqCst),
        2,
        "回填的该是原文，不是空输入"
    );
    assert_eq!(
        fixture.git_ok(&["branch", "--show-current"]),
        "feature/add-login"
    );
}

/// 译文不可用（B18）：清理后仍是中文 → 回名称输入；第二次换成合法译文即成功。
#[test]
fn unusable_translation_returns_to_input_and_retry_succeeds() {
    if common::ensure_console("unusable_translation_returns_to_input_and_retry_succeeds") {
        return;
    }
    let fixture = Fixture::new();
    let chats = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&chats);
    let stub = AiStub::start(move |request| match request.path.as_str() {
        "/chat/completions" => {
            if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                AiReply::ok(chat_body("登录页"))
            } else {
                AiReply::ok(chat_body("add-login"))
            }
        }
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\r");
    pty.expect_screen("分支描述");
    pty.send("登录页");
    pty.send("\r");

    pty.expect_screen_flat("翻译失败：译文不可用（空或仍含非 ASCII）。"); // B18
    pty.send("\r");
    pty.expect_screen_flat(r#"由 "登录页" 翻译为 "add-login""#);
    pty.send("\r");

    pty.expect_screen("Switched to a new branch");
    assert_eq!(pty.exit_code(), 0);
    assert_eq!(chats.load(Ordering::SeqCst), 2);
}

/// 网络类失败（B15，超时同句）：连不上 → 报错、回名称输入；Esc 后无副作用。
#[test]
fn network_failure_reports_and_returns_to_input() {
    if common::ensure_console("network_failure_reports_and_returns_to_input") {
        return;
    }
    let fixture = Fixture::new();
    let dead = AiStub::dead_base_url();
    fixture.write_config(&config_toml(&dead));
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\r");
    pty.expect_screen("分支描述");
    pty.send("添加登录");
    pty.send("\r");

    pty.expect_screen_flat(&format!("翻译失败：连不上 {dead}（网络错误或超时）。")); // B15
    pty.send("\x1b");
    pty.expect_screen("已取消：未做任何改动。");
    assert_eq!(pty.exit_code(), 1);
    assert_eq!(fixture.git_ok(&["branch", "--show-current"]), "main");
}

/// 配置错误（B19）：放行输入、在翻译这一步报出；坏配置原样不动，不静默覆盖。
#[test]
fn broken_config_is_reported_at_translation_time() {
    if common::ensure_console("broken_config_is_reported_at_translation_time") {
        return;
    }
    let fixture = Fixture::new();
    let broken = "provider = \"nope\"\n";
    fixture.write_config(broken);
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\r");
    pty.expect_screen("可直接写中文，自动翻译为英文"); // 配置错误按「已配置」对待

    pty.send("添加登录");
    pty.send("\r");
    pty.expect_screen_flat(&format!(
        "翻译失败：配置错误（provider 取值不认识：`nope`）：{}。",
        fixture.config_path().display()
    )); // B19
    pty.send("\x1b");
    pty.expect_screen("已取消：未做任何改动。");
    assert_eq!(pty.exit_code(), 1);
    assert_eq!(
        fs::read_to_string(fixture.config_path()).expect("坏配置该原地不动"),
        broken
    );
}

/// 无配置零回归：中文仍被 validator 当场拒（B6 指路 bit login）、help 是 B3、无分支落盘。
#[test]
fn without_config_non_ascii_is_rejected_like_v0_1() {
    if common::ensure_console("without_config_non_ascii_is_rejected_like_v0_1") {
        return;
    }
    let fixture = Fixture::new();
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\r");
    pty.expect_screen("分支描述");
    let screen = pty.screen();
    assert!(
        screen.contains("描述性短语，2–5 个词、约 ≤50 字符（软建议）"),
        "未配置时 help 该是 v0.1 的 B3：\n{screen}"
    );
    assert!(
        !screen.contains("可直接写中文"),
        "未配置时 help 不该提翻译：\n{screen}"
    );

    pty.send("添加登录");
    pty.send("\r");
    pty.expect_screen_flat("只允许 a–z 0–9 - .（配置 AI 后可直接写中文：bit login）"); // B6
    pty.send("\x1b");
    pty.expect_screen("已取消：未做任何改动。");
    assert_eq!(pty.exit_code(), 1);
    assert_eq!(
        fixture.git_ok(&["branch", "--format=%(refname:short)"]),
        "main"
    );
}
