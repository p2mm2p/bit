//! 命令面：手写参数解析、帮助正文、诊断文案与退出码。
//!
//! 文案模板冻结在 [命令面 · 帮助、错误与文案语言](https://github.com/p2mm2p/bit/issues/8)，
//! 实现直接抄；三层退出码的语义见 [ADR-0003](../../docs/adr/0003-bit-owns-its-command-surface.md)。
//! 未被 #8 枚举的输入（`--`、组合短选项、任何多余位置参数）不逐条特判，统一落进用法错误出口。

use std::io::IsTerminal;

/// `-V` / `--version` 的全部输出。
pub const VERSION_LINE: &str = concat!("bit ", env!("CARGO_PKG_VERSION"));

/// 用法层失败：还没让 bit 开始做事（无参数、未知命令、未知选项、多余参数）。
pub const EXIT_USAGE: u8 = 2;

/// 运行期失败：bit 开始做事后失败或被取消（取消、非 TTY、无暂存、编辑器非 0）。
pub const EXIT_RUNTIME: u8 = 1;

/// 非 TTY 预检的中文报错。Windows 上 inquire 遇到非 TTY 不会自己失败（会渲染完一直等输入，
/// 见 #5 的实测），这道检查因此由 bit 自己出。
pub const ERR_NOT_A_TTY: &str =
    "错误：stdin 不是终端 —— bit branch 与 bit commit 都是交互式的，请在终端里运行。";

const USAGE_LINES: &str = "  bit branch    选类型 → 输名字 → 确认，然后 git switch -c\n  bit commit    选类型 / scope / breaking → 编辑器补 subject，然后 git commit";

const BRANCH_HELP: &str = "\
bit branch

选类型 → 输名字 → 确认，然后 git switch -c。

用法：
  bit branch

会调用：
  git switch -c <名字>";

const COMMIT_HELP: &str = "\
bit commit

选类型 / scope / breaking → 编辑器补 subject，然后 git commit。

用法：
  bit commit

会调用：
  git commit -F <消息文件> --cleanup=strip";

/// v0.1 的两个子命令。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Branch,
    Commit,
}

impl Command {
    /// 文案里的写法。
    pub fn label(self) -> &'static str {
        match self {
            Command::Branch => "bit branch",
            Command::Commit => "bit commit",
        }
    }

    /// 多余参数时给出的直路（#8 表第 5 行）。
    pub fn direct_hint(self) -> &'static str {
        match self {
            Command::Branch => "要直接建分支请用 git switch -c <名字>。",
            Command::Commit => "要直接提交请用 git commit。",
        }
    }
}

/// 解析结果：`main` 照此分发。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// `-h` / `--help` → 帮助正文到 stdout、退出码 0。
    Help,
    /// `-V` / `--version` → 版本行到 stdout、退出码 0。
    Version,
    /// `bit branch -h` 之类 → 该命令自己那一节到 stdout、退出码 0。
    CommandHelp(Command),
    /// 进入交互（先过 TTY 预检）。
    Run(Command),
    /// 用法层失败 → 诊断到 stderr、退出码 2。
    Usage(UsageError),
}

/// 用法层失败，文案见 [`UsageError::text`]。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsageError {
    /// `bit`
    MissingCommand,
    /// `bit add .` —— 不转发，报错并给出路。
    UnknownCommand(String),
    /// `bit branch foo` / `bit commit -m x` / `bit --help foo`；`None` 指顶层的旗标。
    UnexpectedArg {
        command: Option<Command>,
        arg: String,
    },
}

impl UsageError {
    /// 完整诊断文案（多行，不含结尾换行）。
    pub fn text(&self) -> String {
        match self {
            UsageError::MissingCommand => {
                format!("错误：没有指定命令\n用法：\n{USAGE_LINES}\n运行 bit --help 查看完整帮助。")
            }
            UsageError::UnknownCommand(name) => format!(
                "错误：未知命令 `{name}`\nbit 只有 branch 与 commit 两个命令，都是交互式的，不转发其它 git 命令。\n运行 bit --help 查看用法。"
            ),
            UsageError::UnexpectedArg { command, arg } => match command {
                Some(command) => format!(
                    "错误：{} 不接受参数（`{arg}`）\n{}",
                    command.label(),
                    command.direct_hint()
                ),
                None => format!("错误：bit 不接受参数（`{arg}`）\n运行 bit --help 查看完整帮助。"),
            },
        }
    }
}

/// 手写参数解析（不引 clap）：只认 `branch` / `commit` / `-h|--help` / `-V|--version`。
pub fn parse(args: &[String]) -> Outcome {
    let Some((first, rest)) = args.split_first() else {
        return Outcome::Usage(UsageError::MissingCommand);
    };
    match first.as_str() {
        "-h" | "--help" => flag(Outcome::Help, rest),
        "-V" | "--version" => flag(Outcome::Version, rest),
        "branch" => command(Command::Branch, rest),
        "commit" => command(Command::Commit, rest),
        other => Outcome::Usage(UsageError::UnknownCommand(other.to_string())),
    }
}

fn flag(outcome: Outcome, rest: &[String]) -> Outcome {
    match rest.first() {
        None => outcome,
        Some(extra) => unexpected(None, extra),
    }
}

fn command(command: Command, rest: &[String]) -> Outcome {
    let Some((first, tail)) = rest.split_first() else {
        return Outcome::Run(command);
    };
    if is_help_flag(first) {
        return match tail.first() {
            None => Outcome::CommandHelp(command),
            Some(extra) => unexpected(Some(command), extra),
        };
    }
    unexpected(Some(command), first)
}

fn is_help_flag(arg: &str) -> bool {
    arg == "-h" || arg == "--help"
}

fn unexpected(command: Option<Command>, arg: &str) -> Outcome {
    Outcome::Usage(UsageError::UnexpectedArg {
        command,
        arg: arg.to_string(),
    })
}

/// 帮助正文（#8 表第 2 行）。
pub fn help_text() -> String {
    format!(
        "{VERSION_LINE}

两个交互命令的 git 包装，不做透传。

用法：
{USAGE_LINES}

选项：
  -h, --help     打印这份帮助
  -V, --version  打印版本"
    )
}

/// 子命令自己那一节（#8 Q2：用法 + 一句话 + 会调用哪条 git 命令）。
pub fn command_help(command: Command) -> &'static str {
    match command {
        Command::Branch => BRANCH_HELP,
        Command::Commit => COMMIT_HELP,
    }
}

/// bit 自己的 TTY 预检，只拦两个交互命令；`-h` / `-V` 照常可被管道使用。
pub fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Outcome {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        parse(&args)
    }

    fn usage_text(outcome: Outcome) -> String {
        match outcome {
            Outcome::Usage(error) => error.text(),
            other => panic!("应为用法错误，实际是 {other:?}"),
        }
    }

    #[test]
    fn missing_command_is_a_usage_error() {
        assert_eq!(parse_args(&[]), Outcome::Usage(UsageError::MissingCommand));
        assert_eq!(
            usage_text(parse_args(&[])),
            "错误：没有指定命令\n用法：\n  bit branch    选类型 → 输名字 → 确认，然后 git switch -c\n  bit commit    选类型 / scope / breaking → 编辑器补 subject，然后 git commit\n运行 bit --help 查看完整帮助。"
        );
    }

    #[test]
    fn help_and_version_are_requests() {
        assert_eq!(parse_args(&["-h"]), Outcome::Help);
        assert_eq!(parse_args(&["--help"]), Outcome::Help);
        assert_eq!(parse_args(&["-V"]), Outcome::Version);
        assert_eq!(parse_args(&["--version"]), Outcome::Version);
        assert_eq!(VERSION_LINE, format!("bit {}", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn help_body_matches_the_frozen_table() {
        assert_eq!(
            help_text(),
            format!(
                "{VERSION_LINE}

两个交互命令的 git 包装，不做透传。

用法：
  bit branch    选类型 → 输名字 → 确认，然后 git switch -c
  bit commit    选类型 / scope / breaking → 编辑器补 subject，然后 git commit

选项：
  -h, --help     打印这份帮助
  -V, --version  打印版本"
            )
        );
    }

    #[test]
    fn unknown_command_is_not_forwarded() {
        assert_eq!(
            parse_args(&["add", "."]),
            Outcome::Usage(UsageError::UnknownCommand("add".to_string()))
        );
        assert_eq!(
            usage_text(parse_args(&["add", "."])),
            "错误：未知命令 `add`\nbit 只有 branch 与 commit 两个命令，都是交互式的，不转发其它 git 命令。\n运行 bit --help 查看用法。"
        );
    }

    #[test]
    fn extra_arguments_point_at_git() {
        assert_eq!(
            usage_text(parse_args(&["branch", "foo"])),
            "错误：bit branch 不接受参数（`foo`）\n要直接建分支请用 git switch -c <名字>。"
        );
        assert_eq!(
            usage_text(parse_args(&["commit", "-m", "x"])),
            "错误：bit commit 不接受参数（`-m`）\n要直接提交请用 git commit。"
        );
    }

    #[test]
    fn bare_commands_run_and_help_flags_do_not() {
        assert_eq!(parse_args(&["branch"]), Outcome::Run(Command::Branch));
        assert_eq!(parse_args(&["commit"]), Outcome::Run(Command::Commit));
        assert_eq!(
            parse_args(&["branch", "-h"]),
            Outcome::CommandHelp(Command::Branch)
        );
        assert_eq!(
            parse_args(&["commit", "--help"]),
            Outcome::CommandHelp(Command::Commit)
        );
        assert!(command_help(Command::Branch).contains("git switch -c <名字>"));
        assert!(command_help(Command::Commit).contains("git commit -F <消息文件> --cleanup=strip"));
    }

    #[test]
    fn unenumerated_input_falls_into_the_usage_exit() {
        assert_eq!(
            parse_args(&["--help", "foo"]),
            Outcome::Usage(UsageError::UnexpectedArg {
                command: None,
                arg: "foo".to_string()
            })
        );
        assert_eq!(
            parse_args(&["branch", "--foo"]),
            Outcome::Usage(UsageError::UnexpectedArg {
                command: Some(Command::Branch),
                arg: "--foo".to_string()
            })
        );
        assert_eq!(
            parse_args(&["branch", "-h", "foo"]),
            Outcome::Usage(UsageError::UnexpectedArg {
                command: Some(Command::Branch),
                arg: "foo".to_string()
            })
        );
        assert!(matches!(parse_args(&["--"]), Outcome::Usage(_)));
    }

    #[test]
    fn non_tty_message_matches_the_frozen_table() {
        assert_eq!(
            ERR_NOT_A_TTY,
            "错误：stdin 不是终端 —— bit branch 与 bit commit 都是交互式的，请在终端里运行。"
        );
    }
}
