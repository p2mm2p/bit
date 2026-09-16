//! 命令面的轻断言：只断退出码与流向，不做文案快照。
//!
//! 分层与数值冻结在 [命令面 · 帮助、错误与文案语言](https://github.com/p2mm2p/bit/issues/8)，
//! 依据是 [ADR-0003](../../docs/adr/0003-bit-owns-its-command-surface.md)：
//! 用法层失败 = 2 走 stderr、帮助与版本是「请求」= 0 走 stdout、未知命令与多余参数不转发。

mod common;

use bit::cli::{EXIT_USAGE, VERSION_LINE};
use common::Fixture;

/// 用法层失败 = 2：诊断走 stderr，stdout 保持干净。
#[test]
fn usage_failures_are_two_and_go_to_stderr() {
    let fixture = Fixture::new();
    for args in [
        &[][..],
        &["add", "."][..],
        &["branch", "foo"][..],
        &["commit", "-m", "x"][..],
        &["--help", "foo"][..],
    ] {
        let run = fixture.run_bit(args);
        assert_eq!(run.code, i32::from(EXIT_USAGE), "bit {args:?} 的退出码");
        assert!(
            run.stdout.is_empty(),
            "bit {args:?} 的 stdout 该是空的：{}",
            run.stdout
        );
        assert!(!run.stderr.is_empty(), "bit {args:?} 该把诊断写进 stderr");
    }
}

/// 帮助与版本是「请求」：退出码 0、走 stdout、stderr 干净。
#[test]
fn help_and_version_are_requests() {
    let fixture = Fixture::new();
    for args in [
        &["-h"][..],
        &["--help"][..],
        &["branch", "-h"][..],
        &["commit", "--help"][..],
    ] {
        let run = fixture.run_bit(args);
        assert_eq!(run.code, 0, "bit {args:?} 的退出码");
        assert!(!run.stdout.is_empty(), "bit {args:?} 的帮助该走 stdout");
        assert!(
            run.stderr.is_empty(),
            "bit {args:?} 的 stderr 该是空的：{}",
            run.stderr
        );
    }

    let run = fixture.run_bit(&["-V"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.stdout.trim(), VERSION_LINE);
}

/// 未知命令与多余参数不转发：`bit add .` 说不行，就真的什么都没做。
#[test]
fn unknown_commands_are_not_forwarded_to_git() {
    let fixture = Fixture::new();
    fixture.write("note.md", "不该被暂存的内容\n");

    let run = fixture.run_bit(&["add", "."]);
    assert_eq!(run.code, i32::from(EXIT_USAGE));
    assert_eq!(
        fixture.git_ok(&["diff", "--cached", "--name-only"]),
        "",
        "`bit add .` 不该碰 git"
    );
}
