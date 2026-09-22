//! 用例 3：非 TTY（管道）—— 退出码 1、中文诊断走 stderr、不渲染菜单、不挂起、不做事
//! （[测试 · 测试与 CI 策略](https://github.com/p2mm2p/bit/issues/6) 用例 3）。
//!
//! 这条不需要 pty：它断言的正是「没有被 pty 喂着的 bit」的行为，所以直接管道喂。

mod common;

use bit::cli::{ERR_NOT_A_TTY, EXIT_RUNTIME};
use common::Fixture;

#[test]
fn piped_stdio_is_rejected_before_any_interaction() {
    let fixture = Fixture::new();
    let before = fixture.commit_count();
    // 有暂存内容也不该提交：预检在 TTY 预检之后，压根走不到
    fixture.stage("docs/note.md", "夹具的暂存内容\n");

    for args in [&["branch"][..], &["commit"][..], &["login"][..]] {
        let run = fixture.run_bit(args);
        assert_eq!(run.code, i32::from(EXIT_RUNTIME), "bit {args:?} 的退出码");
        assert!(run.stdout.is_empty(), "诊断不该走 stdout：{}", run.stdout);
        assert!(
            run.stderr.contains(ERR_NOT_A_TTY),
            "bit {args:?} 的 stderr：{}",
            run.stderr
        );
        assert!(
            !run.stderr.contains("分支类型")
                && !run.stderr.contains("提交类型")
                && !run.stderr.contains("提供商"),
            "退出前不该渲染菜单：{}",
            run.stderr
        );
    }

    assert_eq!(
        fixture.git_ok(&["branch", "--format=%(refname:short)"]),
        "main",
        "没有新分支"
    );
    assert_eq!(fixture.commit_count(), before, "没有提交");
}
