//! 解析与调度：所有判断都在 `bit::cli` 里做完，这里只管把结果播出去、把交互流程接上。

use std::process::ExitCode;

use bit::cli::{self, Command, Outcome};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    match cli::parse(&args) {
        Outcome::Help => print_line(&cli::help_text()),
        Outcome::Version => print_line(cli::VERSION_LINE),
        Outcome::CommandHelp(command) => print_line(cli::command_help(command)),
        Outcome::Usage(error) => {
            eprintln!("{}", error.text());
            ExitCode::from(cli::EXIT_USAGE)
        }
        Outcome::Run(command) => run(command),
    }
}

fn run(command: Command) -> ExitCode {
    if !cli::stdin_is_tty() {
        eprintln!("{}", cli::ERR_NOT_A_TTY);
        return ExitCode::from(cli::EXIT_RUNTIME);
    }
    match command {
        Command::Branch => not_implemented(command, "#11"),
        Command::Commit => not_implemented(command, "#12"),
    }
}

fn print_line(text: &str) -> ExitCode {
    println!("{text}");
    ExitCode::SUCCESS
}

fn not_implemented(command: Command, ticket: &str) -> ExitCode {
    eprintln!(
        "提示：{} 的交互流程尚未实现（实现见 {ticket}）。",
        command.label()
    );
    ExitCode::from(cli::EXIT_RUNTIME)
}
