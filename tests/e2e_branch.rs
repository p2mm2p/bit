//! `bit branch` 的 pty e2e。
//!
//! 用例来自 [测试 · 测试与 CI 策略](https://github.com/p2mm2p/bit/issues/6) 的 1 与 4，
//! 外加一条 git 退出码透传（[ADR-0003](../../docs/adr/0003-bit-owns-its-command-surface.md)：
//! git 自身的失败原样透传）。

mod common;

use common::{Fixture, Pty};

/// 用例 1：正向 —— 选类型 → 输名字 → 确认 → `git switch -c` 真建出并切到分支。
#[test]
fn positive_run_creates_and_switches_branch() {
    if common::ensure_console("positive_run_creates_and_switches_branch") {
        return;
    }
    let fixture = Fixture::new();
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\r"); // 默认停在第一项：feature

    pty.expect_screen("分支描述");
    pty.send("Add E2E Demo");
    pty.send("\r");

    // 创建前确认：回显规范化、默认「是」
    pty.expect_screen(r#"由 "Add E2E Demo" 规范化"#);
    pty.expect_screen("feature/add-e2e-demo");
    pty.expect_screen("确认");
    pty.send("\r");

    pty.expect_screen("Switched to a new branch");
    assert_eq!(pty.exit_code(), 0);
    assert_eq!(
        fixture.git_ok(&["branch", "--show-current"]),
        "feature/add-e2e-demo"
    );
}

/// 用例 4：Esc 取消 —— 退出码 1、中文诊断在 stderr、无副作用（没有新分支）。
#[test]
fn escape_at_the_menu_cancels_without_side_effects() {
    if common::ensure_console("escape_at_the_menu_cancels_without_side_effects") {
        return;
    }
    let fixture = Fixture::new();
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\x1b");

    pty.expect_screen("已取消：未做任何改动。");
    assert_eq!(pty.exit_code(), 1);
    assert_eq!(
        fixture.git_ok(&["branch", "--format=%(refname:short)"]),
        "main"
    );
}

/// git 自己的失败原样透传：重名由 git 报、退出码 128 一路透出来（ADR-0003）。
#[test]
fn duplicate_name_passes_gits_exit_code_through() {
    if common::ensure_console("duplicate_name_passes_gits_exit_code_through") {
        return;
    }
    let fixture = Fixture::new();
    fixture.git_ok(&["branch", "feature/dup"]);
    let mut pty = Pty::spawn(&fixture, &["branch"]);

    pty.expect_screen("分支类型");
    pty.send("\r");
    pty.expect_screen("分支描述");
    pty.send("dup");
    pty.send("\r");
    pty.expect_screen("确认");
    pty.send("\r");

    pty.expect_screen("already exists");
    assert_eq!(pty.exit_code(), 128, "git 的退出码原样透传");
    assert_eq!(fixture.git_ok(&["branch", "--show-current"]), "main");
}
