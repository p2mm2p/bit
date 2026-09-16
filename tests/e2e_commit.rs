//! `bit commit` 的 pty e2e（[测试 · 测试与 CI 策略](https://github.com/p2mm2p/bit/issues/6) 用例 2、5、6）。

mod common;

use bit::commit::{self, CommitType};
use common::{Fixture, Pty};

/// 正向用例里编辑器 stub 写下的消息（bit 提交的必须就是它）。
const MESSAGE: &str = "docs(ui): 补 e2e 说明\n\n由 stub 编辑器写下的正文。\n\nRefs: #123";

/// 回环用例里 stub 第二次才写下的消息。
const LOOP_MESSAGE: &str = "feat: 空 subject 之后重开编辑器\n\n第二次拉起编辑器才写下正文。";

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

/// 用例 6：编辑器回环 —— stub 第一次留空 subject，bit 必须重开编辑器，且只提交一次。
#[test]
fn reopens_the_editor_when_the_subject_is_empty() {
    if common::ensure_console("reopens_the_editor_when_the_subject_is_empty") {
        return;
    }
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    fixture.stage("docs/note.md", "e2e 夹具的暂存内容\n");
    fixture.editor_mode("loop");
    fixture.editor_message(&format!("{LOOP_MESSAGE}\n"));

    let mut pty = Pty::spawn(&fixture, &["commit"]);
    pty.expect_screen("提交类型");
    pty.send("\r"); // feat
    pty.expect_screen("作用范围（scope）");
    pty.send("\r"); // scope 留空
    pty.expect_screen("是破坏性变更（breaking change）吗？");
    pty.send("\r"); // 否

    assert_eq!(pty.exit_code(), 0);
    let seed = commit::seed(CommitType::Feat, "", false);
    assert_eq!(
        fixture.editor_calls(),
        vec![seed.clone(), seed],
        "第一次的空 subject 必须逼出第二次拉起，且两次看到的都是同一份原文"
    );
    assert_eq!(fixture.commit_count(), before + 1, "只提交一次");
    assert_eq!(fixture.git_ok(&["log", "-1", "--pretty=%B"]), LOOP_MESSAGE);
}
