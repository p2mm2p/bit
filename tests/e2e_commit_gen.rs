//! `bit commit --gen` 的 pty e2e（[实现 · bit commit --gen](https://github.com/p2mm2p/bit/issues/32) 的验收面）。
//!
//! 行为对照 [行为 · 提交消息生成细则](https://github.com/p2mm2p/bit/issues/24)，
//! 文案逐字对照 [命令面 · v0.2 增补](https://github.com/p2mm2p/bit/issues/27) 的 C1–C17；
//! 网络面全部打给 `tests/common` 的本地 stub 供给（真 HTTP、不联网、不碰真 key）。

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bit::commit::COMMENT_HINT;
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

/// stub 给的合格 draft 字段。
const DRAFT_JSON: &str = r#"{"type":"feat","scope":"ai","breaking":false,"subject":"接通提交消息生成","body":"由本地 stub 供给生成的正文。"}"#;

/// 上面字段经 bit 组装后的提交消息。
const DRAFT_MESSAGE: &str = "feat(ai): 接通提交消息生成\n\n由本地 stub 供给生成的正文。";

/// 正向：一次生成 → C2 展示 → 确认「是」→ 真产生一次提交，编辑器不被拉起。
#[test]
fn generation_confirms_and_commits_the_draft() {
    if common::ensure_console("generation_confirms_and_commits_the_draft") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("src/ai-note.md", "新内容\n");
    fixture.editor_mode("quit");
    let stub = AiStub::start(|request| match request.path.as_str() {
        "/chat/completions" => AiReply::ok(chat_body(DRAFT_JSON)),
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen("正在生成…"); // C1
    pty.expect_screen("按这条消息提交？"); // C3
    let screen = pty.screen();
    assert!(
        screen.contains("feat(ai): 接通提交消息生成"),
        "draft 要展示在确认门前：\n{screen}"
    );
    pty.send("\r"); // 默认「是」

    assert_eq!(pty.exit_code(), 0);
    assert_eq!(fixture.commit_count(), before + 1, "只产生一次提交");
    assert_eq!(fixture.git_ok(&["log", "-1", "--pretty=%B"]), DRAFT_MESSAGE);
    assert_eq!(
        fixture.editor_calls(),
        Vec::<String>::new(),
        "确认「是」不拉编辑器"
    );
}

/// 确认「否」：编辑器预填 draft（附注释提示）；stub 改成合格消息 → 复核通过 → 提交。
#[test]
fn confirm_no_opens_the_editor_with_the_draft_prefilled() {
    if common::ensure_console("confirm_no_opens_the_editor_with_the_draft_prefilled") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("src/ai-note.md", "新内容\n");
    fixture.editor_mode("write");
    fixture.editor_message("fix(ai): 人工改过\n");
    let stub = AiStub::start(|request| match request.path.as_str() {
        "/chat/completions" => AiReply::ok(chat_body(DRAFT_JSON)),
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen("按这条消息提交？");
    pty.send("n"); // 否
    pty.send("\r");

    assert_eq!(pty.exit_code(), 0);
    assert_eq!(fixture.commit_count(), before + 1);
    assert_eq!(
        fixture.git_ok(&["log", "-1", "--pretty=%B"]),
        "fix(ai): 人工改过"
    );
    let calls = fixture.editor_calls();
    assert_eq!(calls.len(), 1, "只拉一次编辑器");
    assert!(
        calls[0].contains("feat(ai): 接通提交消息生成"),
        "预填的该是 draft：{}",
        calls[0]
    );
    assert!(calls[0].contains(COMMENT_HINT), "预填要带同一行注释提示");
}

/// 确认「否」后编辑器未改动就退出 = 放弃（沿 #16 的「未改动退出」语义）。
#[test]
fn quitting_the_editor_untouched_abandons_the_commit() {
    if common::ensure_console("quitting_the_editor_untouched_abandons_the_commit") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("src/ai-note.md", "新内容\n");
    fixture.editor_mode("quit");
    let stub = AiStub::start(|request| match request.path.as_str() {
        "/chat/completions" => AiReply::ok(chat_body(DRAFT_JSON)),
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen("按这条消息提交？");
    pty.send("n");
    pty.send("\r");

    assert_eq!(pty.exit_code(), 1);
    assert_eq!(fixture.commit_count(), before, "不提交");
    assert!(
        pty.screen().contains("已取消：提交消息未改动，未提交。"),
        "未改动退出要有取消文案：\n{}",
        pty.screen()
    );
}

/// 字段异常（type 不在 11 类内）：C4 说明、不开确认门、直接进编辑器；
/// stub 改成合格消息后提交，预填里保留原样的异常 type。
#[test]
fn an_unknown_type_skips_the_confirm_gate_and_opens_the_editor() {
    if common::ensure_console("an_unknown_type_skips_the_confirm_gate_and_opens_the_editor") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("src/ai-note.md", "新内容\n");
    fixture.editor_mode("write");
    fixture.editor_message("docs: 补说明\n");
    let stub = AiStub::start(|request| match request.path.as_str() {
        "/chat/completions" => AiReply::ok(chat_body(
            r#"{"type":"nope","scope":null,"breaking":false,"subject":"补说明","body":null}"#,
        )),
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen_flat("提示：生成的 type 不在 11 类内（`nope`），已带入编辑器复核。"); // C4
    let screen = pty.screen();
    assert!(
        !screen.contains("按这条消息提交？"),
        "字段异常不该开确认门：\n{screen}"
    );
    assert!(
        screen.contains("nope: 补说明"),
        "draft 要原样展示：\n{screen}"
    );

    assert_eq!(pty.exit_code(), 0);
    assert_eq!(fixture.commit_count(), before + 1);
    assert_eq!(
        fixture.git_ok(&["log", "-1", "--pretty=%B"]),
        "docs: 补说明"
    );
    assert!(
        fixture.editor_calls()[0].contains("nope: 补说明"),
        "异常 type 原样带进编辑器"
    );
}

/// 401：C10 生成失败、退出码 1、不提交、不退回手写。
#[test]
fn auth_failure_reports_and_stops() {
    if common::ensure_console("auth_failure_reports_and_stops") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("src/ai-note.md", "新内容\n");
    let stub = AiStub::start(|_request| AiReply::status(401, r#"{"error":{"message":"bad key"}}"#));
    fixture.write_config(&config_toml(stub.base_url()));

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen_flat("错误：生成失败：认证失败（401），请先跑 bit login 检查密钥。"); // C10

    assert_eq!(pty.exit_code(), 1);
    assert_eq!(fixture.commit_count(), before, "失败不提交");
    assert!(fixture.editor_calls().is_empty(), "失败不退回手写");
}

/// 超预算：两段式——先一次分段摘要、再一次最终生成，确认「是」后提交。
#[test]
fn an_over_budget_diff_runs_one_summary_call_then_the_final_generation() {
    if common::ensure_console("an_over_budget_diff_runs_one_summary_call_then_the_final_generation")
    {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    let big_a: String = (0..4_000)
        .map(|index| format!("a 行 {index} 的正文\n"))
        .collect();
    let big_b: String = (0..4_000)
        .map(|index| format!("b 行 {index} 的正文\n"))
        .collect();
    fixture.stage("big-a.txt", &big_a);
    fixture.stage("big-b.txt", &big_b);
    fixture.editor_mode("quit");

    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let stub = AiStub::start(move |request| {
        match request.path.as_str() {
            "/chat/completions" => {
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    // 两条都给出，避免依赖批次的文件顺序。
                    AiReply::ok(chat_body(
                        r#"{"summaries":[{"path":"big-a.txt","summary":"新增大文件 a"},{"path":"big-b.txt","summary":"新增大文件 b"}]}"#,
                    ))
                } else {
                    AiReply::ok(chat_body(DRAFT_JSON))
                }
            }
            _ => AiReply::status(404, "{}"),
        }
    });
    fixture.write_config(&config_toml(stub.base_url()));

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen("按这条消息提交？");
    pty.send("\r");

    assert_eq!(pty.exit_code(), 0);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "一次分段摘要 + 一次最终生成"
    );
    assert_eq!(fixture.commit_count(), before + 1);
    assert_eq!(fixture.git_ok(&["log", "-1", "--pretty=%B"]), DRAFT_MESSAGE);
}

/// 过滤后一个内容文件都不剩：不发正文也要生成（只发 stat 的极端情况）。
#[test]
fn a_fully_filtered_diff_still_generates_from_the_stat_only() {
    if common::ensure_console("a_fully_filtered_diff_still_generates_from_the_stat_only") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("Cargo.lock", "version = 4\n");
    fixture.editor_mode("quit");
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let stub = AiStub::start(move |request| match request.path.as_str() {
        "/chat/completions" => {
            counter.fetch_add(1, Ordering::SeqCst);
            AiReply::ok(chat_body(DRAFT_JSON))
        }
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen("按这条消息提交？");
    pty.send("\r");

    assert_eq!(pty.exit_code(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "只发 stat 也是单次调用");
    assert_eq!(fixture.commit_count(), before + 1);
}

/// C7：编辑器改出的消息仍不合格 → 提示后带原文重开；再未改动即放弃（回环有界）。
#[test]
fn an_unreviewable_edited_message_reopens_and_then_abandons() {
    if common::ensure_console("an_unreviewable_edited_message_reopens_and_then_abandons") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("src/ai-note.md", "新内容\n");
    fixture.editor_mode("write");
    fixture.editor_message("没有 header 的正文\n");
    let stub = AiStub::start(|request| match request.path.as_str() {
        "/chat/completions" => AiReply::ok(chat_body(DRAFT_JSON)),
        _ => AiReply::status(404, "{}"),
    });
    fixture.write_config(&config_toml(stub.base_url()));

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen("按这条消息提交？");
    pty.send("n"); // 否，进编辑器
    pty.send("\r");

    assert_eq!(pty.exit_code(), 1);
    assert_eq!(fixture.commit_count(), before, "不提交");
    assert_eq!(fixture.editor_calls().len(), 2, "复核不过只带原文重开一轮");
    let screen = pty.screen();
    assert!(
        screen.contains(
            "提示：提交消息不合格（subject 为空），已重新打开编辑器；未改动直接退出即放弃提交。"
        ),
        "C7 要上屏：\n{screen}"
    );
    assert!(
        screen.contains("已取消：提交消息未改动，未提交。"),
        "未改动退出要有取消文案：\n{screen}"
    );
}

/// 未配置：C8 在暂存预检之后、读 diff 之前拦下；退出码 1、无请求、无提交。
#[test]
fn without_config_it_stops_before_reading_the_diff() {
    if common::ensure_console("without_config_it_stops_before_reading_the_diff") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("src/ai-note.md", "新内容\n");

    let mut pty = Pty::spawn(&fixture, &["commit", "--gen"]);
    pty.expect_screen_flat("错误：未配置 AI 供给 —— 请先跑 bit login。"); // C8

    assert_eq!(pty.exit_code(), 1);
    assert_eq!(fixture.commit_count(), before, "不提交");
}
