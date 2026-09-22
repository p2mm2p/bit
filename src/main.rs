//! 解析与调度：所有判断都在 `bit::cli` 里做完，这里只管把结果播出去、把交互流程接上。

mod flow;

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
        Command::Branch => flow::branch(),
        Command::Commit => flow::commit(),
        Command::Login => flow::login(),
    }
}

fn print_line(text: &str) -> ExitCode {
    println!("{text}");
    ExitCode::SUCCESS
}
