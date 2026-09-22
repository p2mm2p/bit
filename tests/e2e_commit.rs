//! `bit commit` 的 pty e2e（[测试 · 测试与 CI 策略](https://github.com/p2mm2p/bit/issues/6) 用例 2、5、6，
//! 另加用例 7）：用例 6 的空 subject 回环语义由
//! [修订 · bit commit 空描述回环](https://github.com/p2mm2p/bit/issues/16) 改写为「未改动即放弃」。
//!
//! 夹具默认不写 AI 配置：本文件同时是「无配置零回归」的 e2e 面
//! （[实现 · 测试与 CI 增补（stub 供给）](https://github.com/p2mm2p/bit/issues/33)）。

mod common;

use std::fs;

use bit::commit::{self, CommitType};
use common::{Fixture, Pty};

/// 正向用例里编辑器 stub 写下的消息（bit 提交的必须就是它）。
const MESSAGE: &str = "docs(ui): 补 e2e 说明\n\n由 stub 编辑器写下的正文。\n\nRefs: #123";

/// 改动过但不合格的消息（首行不含 `: `）：bit 该带着它重开编辑器，再未改动才放弃。
const UNUSABLE: &str = "没有冒号空格的正文\n";

/// 用例 2：正向 —— 预置暂存 → 选类 / scope / breaking → 编辑器 stub 写消息 → 真产生一次提交。
#[test]
fn positive_run_commits_what_the_editor_wrote() {
    if common::ensure_console("positive_run_commits_what_the_editor_wrote") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("docs/note.md", "e2e 夹具的暂存内容\n");
    fixture.editor_mode("write");
    fixture.editor_message(&format!("{MESSAGE}\n"));

    let mut pty = Pty::spawn(&fixture, &["commit"]);
    pty.expect_screen("提交类型");
    pty.send("\x1b[B"); // feat → fix
    pty.send("\x1b[B"); // fix → docs
    pty.send("\r");
    pty.expect_screen("作用范围（scope）");
    pty.send("ui");
    pty.send("\r");
    pty.expect_screen("是破坏性变更（breaking change）吗？");
    pty.send("\r"); // 默认否

    assert_eq!(pty.exit_code(), 0);
    assert_eq!(fixture.commit_count(), before + 1, "只产生一次提交");
    assert_eq!(fixture.git_ok(&["log", "-1", "--pretty=%B"]), MESSAGE);
    assert_eq!(
        fixture.editor_calls(),
        vec![commit::seed(CommitType::Docs, "ui", false)],
        "编辑器只被拉起一次，且拿到的正是预填好的种子"
    );
}

/// 用例 5：无暂存预检 —— 退出码 1、不进入交互。
#[test]
fn without_staged_changes_it_stops_before_the_menu() {
    if common::ensure_console("without_staged_changes_it_stops_before_the_menu") {
        return;
    }
    let fixture = Fixture::new();
    let mut pty = Pty::spawn(&fixture, &["commit"]);

    pty.expect_screen("没有暂存内容");
    assert_eq!(pty.exit_code(), 1);
    assert!(
        !pty.screen().contains("提交类型"),
        "预检拦下就不该渲染菜单：\n{}",
        pty.screen()
    );
}

/// 用例 6（#16 改写）：未改动就退出编辑器 = 放弃 —— 退出码 1、不提交、编辑器不被打转。
#[test]
fn quitting_the_editor_untouched_abandons_the_commit() {
    if common::ensure_console("quitting_the_editor_untouched_abandons_the_commit") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("docs/note.md", "e2e 夹具的暂存内容\n");
    fixture.editor_mode("quit");

    let mut pty = Pty::spawn(&fixture, &["commit"]);
    pty.expect_screen("提交类型");
    pty.send("\r"); // feat
    pty.expect_screen("作用范围（scope）");
    pty.send("\r"); // scope 留空
    pty.expect_screen("是破坏性变更（breaking change）吗？");
    pty.send("\r"); // 否

    assert_eq!(pty.exit_code(), 1);
    assert_eq!(fixture.commit_count(), before, "不提交");
    assert_eq!(
        fixture.editor_calls(),
        vec![commit::seed(CommitType::Feat, "", false)],
        "什么都没写就退出：只拉起过一次，不在空描述上继续重开"
    );
    assert!(
        pty.screen().contains("已取消：提交消息未改动，未提交。"),
        "取消文案要上屏：\n{}",
        pty.screen()
    );
}

/// v0.2 惰性读取的回归（#33）：AI 配置坏着也不碰 `bit commit` 交互路径——不读它、不改它。
#[test]
fn broken_ai_config_does_not_touch_the_plain_commit_path() {
    if common::ensure_console("broken_ai_config_does_not_touch_the_plain_commit_path") {
        return;
    }
    let fixture = Fixture::new();
    let broken = "provider = \"nope\"\n";
    fixture.write_config(broken);
    let before = fixture.commit_count();
    fixture.stage("docs/note.md", "e2e 夹具的暂存内容\n");
    fixture.editor_mode("write");
    fixture.editor_message(&format!("{MESSAGE}\n"));

    let mut pty = Pty::spawn(&fixture, &["commit"]);
    pty.expect_screen("提交类型");
    pty.send("\x1b[B"); // feat → fix
    pty.send("\x1b[B"); // fix → docs
    pty.send("\r");
    pty.expect_screen("作用范围（scope）");
    pty.send("ui");
    pty.send("\r");
    pty.expect_screen("是破坏性变更（breaking change）吗？");
    pty.send("\r"); // 默认否

    assert_eq!(pty.exit_code(), 0);
    assert_eq!(fixture.commit_count(), before + 1, "v0.1 路径照常提交");
    assert_eq!(fixture.git_ok(&["log", "-1", "--pretty=%B"]), MESSAGE);
    assert_eq!(
        fs::read_to_string(fixture.config_path()).expect("读配置"),
        broken,
        "非 AI 路径不该触碰配置文件"
    );
}

/// 用例 7（#16）：改动过但不合格 → 带着用户上次的原文重开并提示；再未改动 → 放弃。
#[test]
fn a_changed_but_unusable_message_reopens_with_the_users_text() {
    if common::ensure_console("a_changed_but_unusable_message_reopens_with_the_users_text") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("docs/note.md", "e2e 夹具的暂存内容\n");
    fixture.editor_mode("write");
    fixture.editor_message(UNUSABLE);

    let mut pty = Pty::spawn(&fixture, &["commit"]);
    pty.expect_screen("提交类型");
    pty.send("\r"); // feat
    pty.expect_screen("作用范围（scope）");
    pty.send("\r"); // scope 留空
    pty.expect_screen("是破坏性变更（breaking change）吗？");
    pty.send("\r"); // 否

    assert_eq!(pty.exit_code(), 1);
    assert_eq!(fixture.commit_count(), before, "不提交");
    assert_eq!(
        fixture.editor_calls(),
        vec![
            commit::seed(CommitType::Feat, "", false),
            UNUSABLE.to_string()
        ],
        "第二次拉起看到的必须是用户上次保存的原文（不重置）"
    );
    let screen = pty.screen();
    assert!(
        screen.contains("提示：描述不能为空，已重新打开编辑器；未改动直接退出即放弃提交。"),
        "改动过而仍不合格，重开前要有提示：\n{screen}"
    );
    assert!(
        screen.contains("已取消：提交消息未改动，未提交。"),
        "重开后再未改动即放弃：\n{screen}"
    );
}
