//! 交互流：inquire 负责问、git 负责做，bit 自己只做预检、组装与透传。
//!
//! 文案逐字取自 [命令面](https://github.com/p2mm2p/bit/issues/8) 的冻结表（第 7–13 行），
//! 行为细则见 [行为 · bit branch 细则](https://github.com/p2mm2p/bit/issues/7) 与
//! [行为 · bit commit 细则](https://github.com/p2mm2p/bit/issues/4)：
//! branch 侧是创建前确认（默认是）、「否」回名称输入（预填原文）、规范化时回显原文；
//! commit 侧是暂存预检、编辑器一律问 `git var GIT_EDITOR`、校验委派给
//! `git stripspace --strip-comments`、不合格带着用户上次的原文重开编辑器。
//! 两侧的 git 调用都把 stdout/stderr 与退出码原样透传。交互界面渲染在 stderr（inquire 的现状），
//! stdout 只承载 git 的输出（ADR-0003）。

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use bit::branch::{self, BranchType};
use bit::cli::EXIT_RUNTIME;
use bit::commit::{self, CommitType};
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

/// `bit commit`：预检暂存 → 选类型 → 输 scope → breaking → 编辑器补 subject → `git commit -F`。
pub fn commit() -> ExitCode {
    if let Err(code) = ensure_staged() {
        return code;
    }

    let selected = match Select::new("提交类型", CommitType::ALL.to_vec())
        .with_page_size(CommitType::ALL.len())
        .prompt_skippable()
    {
        Ok(Some(selected)) => selected,
        Ok(None) => return cancelled(),
        Err(error) => return inquire_failed(error),
    };

    let scope = match Text::new("作用范围（scope）")
        .with_placeholder("可留空，例如 ui")
        .with_validator(|input: &str| match commit::scope(input) {
            Ok(_) => Ok(Validation::Valid),
            Err(message) => Ok(Validation::Invalid(message.into())),
        })
        .prompt_skippable()
    {
        Ok(Some(scope)) => scope,
        Ok(None) => return cancelled(),
        Err(error) => return inquire_failed(error),
    };
    let scope = commit::scope(&scope).expect("validator 用的就是这个函数");

    let breaking = match Confirm::new("是破坏性变更（breaking change）吗？")
        .with_default(false)
        .prompt_skippable()
    {
        Ok(Some(breaking)) => breaking,
        Ok(None) => return cancelled(),
        Err(error) => return inquire_failed(error),
    };

    edit_until_acceptable(selected, &scope, breaking)
}

/// 种子写进 git 自己的消息草稿文件（ADR-0001），用 `git var GIT_EDITOR` 指到的编辑器补 subject；
/// 不合格的回环有界（#16 修订 #4 的「空则重开」）：消息与上一轮逐字相同 = 用户没打算写 → 放弃，
/// 改动过才带着原文重开。
fn edit_until_acceptable(ty: CommitType, scope: &str, breaking: bool) -> ExitCode {
    let path = match message_path() {
        Ok(path) => path,
        Err(code) => return code,
    };
    if let Err(error) = fs::write(&path, commit::seed(ty, scope, breaking)) {
        eprintln!("错误：无法写入消息文件（{error}）。");
        return ExitCode::from(EXIT_RUNTIME);
    }
    let editor = match git_editor() {
        Ok(editor) => editor,
        Err(code) => return code,
    };
    // 基线用同一个尺子量：种子经 `git stripspace --strip-comments` 后长什么样
    let mut previous = match stripped_message(&path) {
        Ok(message) => message,
        Err(code) => return code,
    };
    loop {
        if let Err(code) = open_editor(&editor, &path) {
            return code;
        }
        let message = match stripped_message(&path) {
            Ok(message) => message,
            Err(code) => return code,
        };
        match commit::verdict(&previous, &message) {
            commit::Verdict::Accept => return git_commit(&path),
            commit::Verdict::Cancel => {
                eprintln!("已取消：提交消息未改动，未提交。");
                return ExitCode::from(EXIT_RUNTIME);
            }
            commit::Verdict::Reopen => {
                previous = message;
                eprintln!("提示：描述不能为空，已重新打开编辑器；未改动直接退出即放弃提交。");
            }
        }
    }
}

/// 暂存预检（#4 第 5 步）：退出码 1 继续、0 拦下（stderr、退出码 1、不进交互），
/// 其余按 git 自身的失败原样透传（非仓库时是 129 的用法报错，ADR-0003）。
fn ensure_staged() -> Result<(), ExitCode> {
    match Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .status()
    {
        Ok(status) => match status.code() {
            Some(code) => match commit::staged_check(code) {
                commit::Staged::Present => Ok(()),
                commit::Staged::Missing => {
                    eprintln!("错误：没有暂存内容 —— 请先 git add。");
                    Err(ExitCode::from(EXIT_RUNTIME))
                }
                commit::Staged::Git => Err(exit_code(code)),
            },
            None => {
                eprintln!("错误：git diff 被信号终止。");
                Err(ExitCode::from(EXIT_RUNTIME))
            }
        },
        Err(error) => {
            eprintln!("错误：无法运行 git（{error}）。");
            Err(ExitCode::from(EXIT_RUNTIME))
        }
    }
}

/// 消息文件 = git 的 `COMMIT_EDITMSG`（ADR-0001）：位置与生命周期都交给 git，
/// 绝对路径由 `git rev-parse --path-format=absolute --git-path` 解析，linked worktree 下也正确。
fn message_path() -> Result<PathBuf, ExitCode> {
    let path = git_capture(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "COMMIT_EDITMSG",
    ])?;
    Ok(PathBuf::from(path.trim()))
}

/// 编辑器一律问 git（ADR-0001），不自行复刻 `GIT_EDITOR` > `core.editor` > `VISUAL` > `EDITOR`。
fn git_editor() -> Result<String, ExitCode> {
    Ok(git_capture(&["var", "GIT_EDITOR"])?.trim().to_string())
}

/// 捕获 git 的 stdout；git 失败时把它的 stderr 原样放出来、退出码原样透传（ADR-0003）。
fn git_capture(args: &[&str]) -> Result<String, ExitCode> {
    let output = match Command::new("git").args(args).output() {
        Ok(output) => output,
        Err(error) => {
            eprintln!("错误：无法运行 git（{error}）。");
            return Err(ExitCode::from(EXIT_RUNTIME));
        }
    };
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(exit_code(
            output.status.code().unwrap_or(i32::from(EXIT_RUNTIME)),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 「git 将保存的消息」= `git stripspace --strip-comments` 的输出（ADR-0001），
/// 清洗逻辑不自己重写；消息文件直接接到它的 stdin。
fn stripped_message(path: &Path) -> Result<String, ExitCode> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("错误：无法读取消息文件（{error}）。");
            return Err(ExitCode::from(EXIT_RUNTIME));
        }
    };
    let output = match Command::new("git")
        .args(["stripspace", "--strip-comments"])
        .stdin(Stdio::from(file))
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("错误：无法运行 git（{error}）。");
            return Err(ExitCode::from(EXIT_RUNTIME));
        }
    };
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(exit_code(
            output.status.code().unwrap_or(i32::from(EXIT_RUNTIME)),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 编辑器非 0 退出（vim 的 `:cq` 等）按取消收场（#4）：一句中文、退出码 1、不调用 git commit。
fn open_editor(editor: &str, path: &Path) -> Result<(), ExitCode> {
    match editor_command(editor, path).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => {
            eprintln!("已取消：编辑器非 0 退出，未提交。");
            Err(ExitCode::from(EXIT_RUNTIME))
        }
        Err(error) => {
            eprintln!("错误：无法运行编辑器 `{editor}`（{error}）。");
            Err(ExitCode::from(EXIT_RUNTIME))
        }
    }
}

/// `GIT_EDITOR` 可能是一整条命令行（`code --wait`），按 git 的做法经 shell 拉起。
#[cfg(windows)]
fn editor_command(editor: &str, path: &Path) -> Command {
    use std::os::windows::process::CommandExt;

    let mut command = Command::new("cmd");
    command.arg("/C");
    command.raw_arg(format!("{editor} \"{}\"", path.display()));
    command
}

#[cfg(not(windows))]
fn editor_command(editor: &str, path: &Path) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg(editor)
        .arg(path);
    command
}

/// 提交：git 的 stdout/stderr 与退出码原样透传，bit 不追加成功文案（#4、ADR-0003）。
fn git_commit(path: &Path) -> ExitCode {
    match Command::new("git")
        .arg("commit")
        .arg("-F")
        .arg(path)
        .arg("--cleanup=strip")
        .status()
    {
        Ok(status) => match status.code() {
            Some(code) => exit_code(code),
            None => {
                eprintln!("错误：git commit 被信号终止。");
                ExitCode::from(EXIT_RUNTIME)
            }
        },
        Err(error) => {
            eprintln!("错误：无法运行 git（{error}）。");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}

/// git 的退出码原样透传（ADR-0003）：0–255 照搬，越界按 bit 的运行期失败收场。
fn exit_code(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(EXIT_RUNTIME))
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
