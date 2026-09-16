//! 交互流：inquire 负责问、git 负责做，bit 自己只做预检、组装与透传。
//!
//! 文案逐字取自 [命令面](https://github.com/p2mm2p/bit/issues/8) 的冻结表（第 7、10–13 行），
//! 行为细则见 [行为 · bit branch 细则](https://github.com/p2mm2p/bit/issues/7)：
//! 创建前确认（默认是）、「否」回名称输入（预填原文）、规范化时回显原文、
//! git 的 stdout/stderr 与退出码原样透传。交互界面渲染在 stderr（inquire 的现状），
//! stdout 只承载 git 的输出（ADR-0003）。

use std::process::{Command, ExitCode};

use bit::branch::{self, BranchType};
use bit::cli::EXIT_RUNTIME;
use inquire::validator::Validation;
use inquire::{Confirm, InquireError, Select, Text};

/// `bit branch`：选类型 → 输名字 → 确认 → `git switch -c`。
pub fn branch() -> ExitCode {
    let selected = match Select::new("分支类型", BranchType::ALL.to_vec())
        .with_page_size(BranchType::ALL.len())
        .prompt_skippable()
    {
        Ok(Some(selected)) => selected,
        Ok(None) => return cancelled(),
        Err(error) => return inquire_failed(error),
    };

    let mut raw_input = String::new();
    loop {
        let raw = match Text::new("分支描述")
            .with_placeholder("add-oauth-login")
            .with_help_message("描述性短语，2–5 个词、约 ≤50 字符（软建议）")
            .with_initial_value(&raw_input)
            .with_validator(move |input: &str| match branch::resolve(input, selected) {
                Ok(_) => Ok(Validation::Valid),
                Err(error) => Ok(Validation::Invalid(error.message().into())),
            })
            .prompt_skippable()
        {
            Ok(Some(raw)) => raw,
            Ok(None) => return cancelled(),
            Err(error) => return inquire_failed(error),
        };
        let resolved = branch::resolve(&raw, selected).expect("validator 用的就是这个函数");

        let message = format!("将创建并切换到 {}，确认？", resolved.name);
        let normalized_note = format!("由 \"{raw}\" 规范化");
        let mut confirm = Confirm::new(&message).with_default(true);
        if resolved.changed {
            confirm = confirm.with_help_message(&normalized_note);
        }
        match confirm.prompt_skippable() {
            Ok(Some(true)) => return switch(&resolved.name),
            Ok(Some(false)) => raw_input = raw,
            Ok(None) => return cancelled(),
            Err(error) => return inquire_failed(error),
        }
    }
}

/// 创建并切换：只让 git 说话（#7 第 6 步）——重名、非仓库、detached HEAD、空仓库都不预检，
/// stdout/stderr 与退出码原样透传，bit 不追加成功文案。
fn switch(name: &str) -> ExitCode {
    match Command::new("git").args(["switch", "-c", name]).status() {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(EXIT_RUNTIME)),
            None => {
                eprintln!("错误：git switch -c 被信号终止。");
                ExitCode::from(EXIT_RUNTIME)
            }
        },
        Err(error) => {
            eprintln!("错误：无法运行 git（{error}）。");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}

/// Esc 取消（任一步骤，含确认）：stderr 一句中文、退出码 1、不调用 git、无副作用。
fn cancelled() -> ExitCode {
    eprintln!("已取消：未做任何改动。");
    ExitCode::from(EXIT_RUNTIME)
}

/// Ctrl-C 由信号语义收场、bit 不加文案（#8 Q5）；其余交互异常是 bit 的运行期失败。
fn inquire_failed(error: InquireError) -> ExitCode {
    match error {
        InquireError::OperationInterrupted => ExitCode::from(130),
        other => {
            eprintln!("错误：交互界面出错（{other}）。");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}
