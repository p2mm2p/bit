//! `bit login` 的 pty e2e（#30 的验收面）：菜单选择、掩码输入、验证成功 / 失败、Esc 取消。
//!
//! 网络面全部打给 `tests/common` 的本地 stub 供给（真 HTTP、不联网、不碰真 key），
//! 文案逐字对照 [原型 · bit login 向导交互与文案](https://github.com/p2mm2p/bit/issues/26)
//! 与 [命令面 · v0.2 增补](https://github.com/p2mm2p/bit/issues/27) 的登录节。

mod common;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{AiReply, AiStub, CHAT_OK_BODY, Fixture, Pty, models_body};

/// 正向：自定义家 → base_url 手输 → 掩码 key → 模型菜单 → 验证通过 → 写盘 + 三行摘要。
#[test]
fn happy_path_custom_provider_writes_config() {
    if common::ensure_console("happy_path_custom_provider_writes_config") {
        return;
    }
    let fixture = Fixture::new();
    let stub = AiStub::start(|request| match request.path.as_str() {
        "/models" => AiReply::ok(models_body(&["stub-model-a", "stub-model-b"])),
        "/chat/completions" => AiReply::ok(CHAT_OK_BODY),
        _ => AiReply::status(404, "{}"),
    });
    let mut pty = Pty::spawn(&fixture, &["login"]);

    pty.expect_screen("提供商");
    pty.send(&"\x1b[B".repeat(8)); // 光标挪到第 9 项「自定义 OpenAI 兼容」
    pty.send("\r");

    pty.expect_screen("base_url");
    pty.send(stub.base_url());
    pty.send("\r");

    pty.expect_screen("API Key");
    pty.send("sk-test-123456");
    pty.expect_screen("API Key **************"); // 14 个字符 → 14 个星号
    let screen = pty.screen();
    assert!(
        !screen.contains("sk-test-123456"),
        "掩码输入不该把明文留在屏幕上：\n{screen}"
    );
    pty.send("\r");

    pty.expect_screen("stub-model-a"); // 模型菜单
    pty.send("\r");

    pty.expect_screen("已保存 AI 供给：自定义 OpenAI 兼容 / stub-model-a");
    pty.expect_screen(&format!("端点：{}", stub.base_url()));
    pty.expect_screen(&format!("配置文件：{}", fixture.config_path().display()));
    let screen = pty.screen();
    assert!(
        screen.contains("正在获取模型列表…"),
        "缺拉列表进度行：\n{screen}"
    );
    assert!(
        screen.contains("正在验证连通性…"),
        "缺验证进度行：\n{screen}"
    );
    assert_eq!(pty.exit_code(), 0);
    assert_eq!(
        fs::read_to_string(fixture.config_path()).expect("login 该写出配置文件"),
        format!(
            "# bit 的 AI 供给 —— 由 bit login 生成，可手工编辑。\n\
             # 密钥为明文，请勿把本文件提交进仓库。\n\
             provider = \"custom\"\n\
             base_url = \"{}\"\n\
             model = \"stub-model-a\"\n\
             api_key = \"sk-test-123456\"\n",
            stub.base_url()
        ),
        "落盘形状按 config::write 的固定模板"
    );
}

/// Esc 取消：退出码 1、中文诊断、不写配置（#26 拍板：任意一步都取消）。
#[test]
fn escape_at_the_provider_menu_cancels_without_side_effects() {
    if common::ensure_console("escape_at_the_provider_menu_cancels_without_side_effects") {
        return;
    }
    let fixture = Fixture::new();
    let mut pty = Pty::spawn(&fixture, &["login"]);

    pty.expect_screen("提供商");
    pty.send("\x1b");

    pty.expect_screen("已取消：未做任何改动。");
    assert_eq!(pty.exit_code(), 1);
    assert!(!fixture.config_path().exists(), "取消不该写配置");
}

/// 验证 401：回 key 输入（不退出）；换 key 重试后成功、写盘落新 key。
#[test]
fn auth_failure_returns_to_key_and_retry_succeeds() {
    if common::ensure_console("auth_failure_returns_to_key_and_retry_succeeds") {
        return;
    }
    let fixture = Fixture::new();
    let chats = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&chats);
    let stub = AiStub::start(move |request| match request.path.as_str() {
        "/models" => AiReply::ok(models_body(&["stub-model-a"])),
        "/chat/completions" => {
            if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                AiReply::status(401, r#"{"error":{"message":"bad key"}}"#)
            } else {
                AiReply::ok(CHAT_OK_BODY)
            }
        }
        _ => AiReply::status(404, "{}"),
    });
    let mut pty = Pty::spawn(&fixture, &["login"]);

    pty.expect_screen("提供商");
    pty.send(&"\x1b[B".repeat(8));
    pty.send("\r");
    pty.expect_screen("base_url");
    pty.send(stub.base_url());
    pty.send("\r");
    pty.expect_screen("API Key");
    pty.send("bad-key-1234");
    pty.send("\r");
    pty.expect_screen("stub-model-a");
    pty.send("\r");

    pty.expect_screen("认证失败（401）：API Key 无效或已过期，请重新输入。");
    pty.expect_screen("? API Key"); // 新的活动提示，不是提交过的那行
    pty.send("good-key-1234");
    pty.send("\r");
    pty.expect_screen("stub-model-a");
    pty.send("\r");

    pty.expect_screen("已保存 AI 供给：自定义 OpenAI 兼容 / stub-model-a");
    assert_eq!(pty.exit_code(), 0);
    let written = fs::read_to_string(fixture.config_path()).expect("重试成功该写盘");
    assert!(
        written.contains("api_key = \"good-key-1234\""),
        "落盘的该是重试后的 key：\n{written}"
    );
}

/// 验证 404：回模型输入、复用已拉取的列表；换一个模型后成功。
#[test]
fn model_404_returns_to_model_menu_and_retry_succeeds() {
    if common::ensure_console("model_404_returns_to_model_menu_and_retry_succeeds") {
        return;
    }
    let fixture = Fixture::new();
    let chats = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&chats);
    let stub = AiStub::start(move |request| match request.path.as_str() {
        "/models" => AiReply::ok(models_body(&["stub-model-a", "stub-model-b"])),
        "/chat/completions" => {
            if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                AiReply::status(404, r#"{"error":{"message":"no such model"}}"#)
            } else {
                AiReply::ok(CHAT_OK_BODY)
            }
        }
        _ => AiReply::status(404, "{}"),
    });
    let mut pty = Pty::spawn(&fixture, &["login"]);

    pty.expect_screen("提供商");
    pty.send(&"\x1b[B".repeat(8));
    pty.send("\r");
    pty.expect_screen("base_url");
    pty.send(stub.base_url());
    pty.send("\r");
    pty.expect_screen("API Key");
    pty.send("sk-test-123456");
    pty.send("\r");
    pty.expect_screen("stub-model-a");
    pty.send("\r"); // 先选 a → 404

    pty.expect_screen("模型不可用（404）：stub-model-a。请换一个模型。");
    pty.send("\x1b[B"); // 名单还在，挪到 b
    pty.send("\r");

    pty.expect_screen("已保存 AI 供给：自定义 OpenAI 兼容 / stub-model-b");
    assert_eq!(pty.exit_code(), 0);
}

/// 网络错误：/models 静默回退手输；验证时连不上 → 报错退出 1、不写盘。
#[test]
fn network_failure_exits_one_after_manual_model() {
    if common::ensure_console("network_failure_exits_one_after_manual_model") {
        return;
    }
    let fixture = Fixture::new();
    let dead = AiStub::dead_base_url();
    let mut pty = Pty::spawn(&fixture, &["login"]);

    pty.expect_screen("提供商");
    pty.send(&"\x1b[B".repeat(8));
    pty.send("\r");
    pty.expect_screen("base_url");
    pty.send(&dead);
    pty.send("\r");
    pty.expect_screen("API Key");
    pty.send("sk-test-123456");
    pty.send("\r");

    pty.expect_screen("未能获取模型列表，请手动输入模型名（咨询你的提供商）");
    pty.send("some-model");
    pty.send("\r");

    pty.expect_screen(&format!("错误：连不上 {dead}（无法建立连接）。"));
    pty.expect_screen("检查网络或代理设置，稍后重跑 bit login。");
    assert_eq!(pty.exit_code(), 1);
    assert!(!fixture.config_path().exists(), "失败不该写配置");
}

/// 重配：当前值作默认 —— 顶部上下文、provider 光标、key 留空保持、模型光标落当前值；
/// 手改过的 base_url 不被预置端点覆盖。
#[test]
fn reconfig_keeps_key_and_current_base_url() {
    if common::ensure_console("reconfig_keeps_key_and_current_base_url") {
        return;
    }
    let fixture = Fixture::new();
    let stub = AiStub::start(|request| match request.path.as_str() {
        "/models" => AiReply::ok(models_body(&["stub-model-a", "stub-model-b"])),
        "/chat/completions" => AiReply::ok(CHAT_OK_BODY),
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&format!(
        "# bit 的 AI 供给 —— 由 bit login 生成，可手工编辑。\n\
         # 密钥为明文，请勿把本文件提交进仓库。\n\
         provider = \"deepseek\"\n\
         base_url = \"{}\"\n\
         model = \"stub-model-b\"\n\
         api_key = \"sk-proj-abc12347890xyz\"\n",
        stub.base_url()
    ));
    let mut pty = Pty::spawn(&fixture, &["login"]);

    pty.expect_screen("当前配置：DeepSeek / stub-model-b");
    pty.expect_screen("> DeepSeek｜国内直连"); // 菜单光标落在当前 provider
    pty.send("\r");

    pty.expect_screen("已配置（sk-…0xyz），留空保持不变");
    pty.send("\r"); // 留空 = 保持当前 key
    pty.expect_screen("> stub-model-b"); // 模型光标落在当前值
    pty.send("\r");

    pty.expect_screen("已保存 AI 供给：DeepSeek / stub-model-b");
    assert_eq!(pty.exit_code(), 0);
    let written = fs::read_to_string(fixture.config_path()).expect("重配该写回文件");
    assert!(
        written.contains(&format!("base_url = \"{}\"", stub.base_url())),
        "重配不该把当前端点换成预置端点：\n{written}"
    );
    assert!(
        written.contains("api_key = \"sk-proj-abc12347890xyz\""),
        "留空该保持当前 key：\n{written}"
    );
}

/// L1：启动即读到坏配置 —— 报路径与要点、退出 1、文件原样不动。
#[test]
fn broken_config_is_reported_without_overwrite() {
    if common::ensure_console("broken_config_is_reported_without_overwrite") {
        return;
    }
    let fixture = Fixture::new();
    let broken = "# 手改坏的配置\nprovider = \"nope\"\n";
    fixture.write_config(broken);

    let mut pty = Pty::spawn(&fixture, &["login"]);
    // 文案比 80 列宽，vt100 网格会折行、折点随临时路径长度变化：先拼回一行再断言全句
    pty.expect_screen_flat(&format!(
        "错误：配置文件不可用：{}（provider 取值不认识：`nope`）。修好或删除后重跑 bit login。",
        fixture.config_path().display()
    ));
    assert_eq!(pty.exit_code(), 1);
    assert_eq!(
        fs::read_to_string(fixture.config_path()).expect("坏配置该原地不动"),
        broken
    );
}
