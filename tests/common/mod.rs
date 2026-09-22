//! e2e 夹具：临时仓库 + 隔离环境 + 确定性编辑器 stub + pty 驱动。
//!
//! 策略来自 [测试 · 测试与 CI 策略](https://github.com/p2mm2p/bit/issues/6)，
//! 实现约束与两条 ConPTY 实测坑来自 [实现 · pty e2e 与三平台 CI](https://github.com/p2mm2p/bit/issues/13)：
//! 每用例独立临时仓库、`GIT_EDITOR` 固定 stub、pty 尺寸显式 80×24、Enter 用 `\r`、
//! 断言打在 `vt100` 解析后的屏幕网格上、不用固定 sleep、失败即 kill（防 pty / fd 泄漏）。
//! 环境一律先 `env_clear()` 再整份搬当前环境：`conpty` 只拼显式 set 过的变量，
//! 用 `.env(...)` 单加一个隔离变量会把 `PATH` 一起丢掉（bit 里的 `git` 会变成 program not found）。

#![allow(dead_code)]

use std::fs;
use std::io;
#[cfg(windows)]
use std::io::IsTerminal;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use expectrl::Session;
use expectrl::session::OsSession;

/// `cargo` 交给集成测试的 bit 可执行文件。
pub const BIT: &str = env!("CARGO_BIN_EXE_bit");

/// 等屏幕、等退出码的上限（#6：10–15s，不用固定 sleep）。
pub const TIMEOUT: Duration = Duration::from_secs(15);

/// pty 尺寸（#13：不设会读到 0×0 或跟着驱动控制台走，渲染与断言都会漂）。
const COLS: u16 = 80;
const ROWS: u16 = 24;

/// 轮询间隔：等屏幕、等退出码都用它，避免空转。
const POLL: Duration = Duration::from_millis(10);

/// 提示出现后再等这么久没有新输出，才认为「界面稳住了、可以发按键」。
const SETTLE: Duration = Duration::from_millis(120);
/// 等「稳住」的上限：一直在刷新的界面不能把等待拖死。
const SETTLE_LIMIT: Duration = Duration::from_secs(2);

/// 「正在隐藏控制台里重跑」的标记。
const CHILD_ENV: &str = "BIT_E2E_IN_CONSOLE";
/// 子进程往哪个文件写诊断（它的输出躺在隐藏控制台里，父进程看不见）。
const REPORT_ENV: &str = "BIT_E2E_REPORT";

/// Windows 上驱动自己得带一份**真控制台**，否则 e2e 会假失败。
///
/// `conpty` 不设 `STARTF_USESTDHANDLES`，给 bit 的标准句柄是**驱动标准句柄的副本**（实测：
/// 驱动被管道喂——本机自动化、cargo 采集输出、部分 CI 形态——时，pty 里子进程的 stdout
/// `GetFileType` 就是 `FILE_TYPE_PIPE`，而驱动自己连控制台都没有）。管道句柄的副本在 bit
/// 手里不是控制台，于是 TTY 预检把自己拦下、pty 里的输入也送不到它手里（#13 的第一条实测坑）。
///
/// 对策：驱动没有控制台标准流时，把同一份用例重起一遍，交给 `Start-Process` 开的那份
/// **隐藏控制台**（它走 ShellExecute，不设 `STARTF_USESTDHANDLES`，子进程于是拿到新控制台的
/// 标准句柄——与 #11 / #12 本机冒烟用的手法同源）。bit 拿到的句柄副本于是是控制台句柄，
/// 落在 bit 自己的那个伪控制台上。子进程的输出躺在隐藏控制台里看不见，失败信息经报告文件回传。
/// `cmd /C start` 不行：它把自己的管道句柄一并传下去（实测）。
/// 驱动自己就有控制台标准流时（人在终端里跑 `cargo test`）不绕这一圈。
///
/// Unix 走 `forkpty`，pty 就是子进程自己的终端，与驱动的标准流无关，这里什么都不做。
/// 返回 `true` 表示「本用例已经交给带控制台的子进程跑完并且通过」，调用方应当直接返回；
/// 返回 `false` 表示照常往下跑（Unix、驱动自己就带控制台、或已经在那份子进程里）。
pub fn ensure_console(test_name: &str) -> bool {
    #[cfg(unix)]
    {
        let _ = test_name;
        false
    }

    #[cfg(windows)]
    {
        if std::env::var_os(CHILD_ENV).is_some() {
            child_setup();
            return false;
        }
        if console_stdio() {
            return false;
        }

        let report = std::env::temp_dir().join(format!("bit-e2e-{test_name}.txt"));
        let _ = fs::remove_file(&report);
        let exe = std::env::current_exe().expect("测试可执行文件");
        let mut child = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(format!(
                "$p = Start-Process -FilePath '{exe}' -ArgumentList '--exact','{test_name}','--nocapture' -Wait -PassThru -WindowStyle Hidden; exit $p.ExitCode",
                exe = exe.display(),
            ))
            .env(CHILD_ENV, "1")
            .env(REPORT_ENV, &report)
            .spawn()
            .expect("带控制台重起驱动进程");
        let code = wait_child(&mut child, TIMEOUT * 4);
        let report = fs::read_to_string(&report).unwrap_or_else(|_| "（没有报告文件）".to_string());
        let _ = fs::remove_file(&report);
        assert_eq!(code, Some(0), "隐藏控制台里的用例失败：\n{report}");
        true
    }
}

/// 子进程侧：把「有没有拿到控制台标准流」写进报告，并让 panic 也进报告
/// （它的输出躺在隐藏控制台里，父进程看不见）。
#[cfg(windows)]
fn child_setup() {
    let Some(report) = std::env::var_os(REPORT_ENV) else {
        return;
    };
    let _ = fs::write(
        &report,
        format!(
            "控制台标准流：stdin={} stdout={} stderr={}\n",
            io::stdin().is_terminal(),
            io::stdout().is_terminal(),
            io::stderr().is_terminal()
        ),
    );
    std::panic::set_hook(Box::new(move |info| {
        let _ = fs::OpenOptions::new()
            .append(true)
            .open(&report)
            .and_then(|mut file| {
                std::io::Write::write_all(&mut file, format!("{info}").as_bytes())
            });
    }));
}

/// 驱动的三个标准流是否都落在控制台上：任一是管道 / 文件就换控制台重起。
#[cfg(windows)]
fn console_stdio() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal() && io::stderr().is_terminal()
}

/// 带期限等一个子进程，超时即 kill；返回退出码。
fn wait_child(child: &mut Child, timeout: Duration) -> Option<i32> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().expect("等子进程") {
            Some(status) => return status.code(),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                return None;
            }
            None => std::thread::sleep(POLL),
        }
    }
}

/// 一次非 pty 运行的产物（用法层与预检的退出码级断言用）。
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// 一个用例的沙箱：仓库、编辑器 stub 的数据目录、以及两者共用的隔离环境。
pub struct Fixture {
    dir: tempfile::TempDir,
    repo: PathBuf,
    stub_dir: PathBuf,
    calls: PathBuf,
}

impl Fixture {
    /// 建临时根（`repo/` 与 `editor/` 并列），起一个 `main` 分支、有一次提交的仓库，
    /// 并把编辑器 stub 的数据目录摆好——`GIT_EDITOR` 指到编译出来的 stub 可执行文件，
    /// 真实编辑器一律不会被拉起。
    pub fn new() -> Fixture {
        let dir = tempfile::tempdir().expect("建临时目录");
        let repo = dir.path().join("repo");
        let stub_dir = dir.path().join("editor");
        let calls = stub_dir.join("calls");
        fs::create_dir_all(&repo).expect("建仓库目录");
        fs::create_dir_all(&calls).expect("建 stub 记录目录");

        let fixture = Fixture {
            dir,
            repo,
            stub_dir,
            calls,
        };
        fixture.editor_mode("write");

        fixture.git_ok(&["init", "-b", "main"]);
        fixture.git_ok(&["config", "user.name", "bit e2e"]);
        fixture.git_ok(&["config", "user.email", "bit-e2e@example.com"]);
        // 先垫一次提交：`main` 得是真实存在的分支，重名与「什么都没发生」才断得准
        fixture.write("README.md", "夹具仓库\n");
        fixture.git_ok(&["add", "README.md"]);
        fixture.git_ok(&["commit", "-m", "chore: 夹具初始提交"]);
        fixture
    }

    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// 往仓库里写一个文件（不暂存）。
    pub fn write(&self, rel: &str, contents: &str) {
        let path = self.repo.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("建文件父目录");
        }
        fs::write(&path, contents).expect("写仓库文件");
    }

    /// 写一个文件并 `git add`（`bit commit` 的暂存预检就指着它）。
    pub fn stage(&self, rel: &str, contents: &str) {
        self.write(rel, contents);
        self.git_ok(&["add", rel]);
    }

    /// 仓库里跑 git，stdout 是 trim 过的（夹具自检用，失败即 panic）。
    pub fn git_ok(&self, args: &[&str]) -> String {
        let mut cmd = Command::new("git");
        cmd.args(args);
        self.isolate(&mut cmd);
        let output = cmd.output().expect("跑 git");
        assert!(
            output.status.success(),
            "git {args:?} 失败：{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// 仓库里跑 git，只关心成不成功（断「什么都没发生」用）。
    pub fn git_succeeds(&self, args: &[&str]) -> bool {
        let mut cmd = Command::new("git");
        cmd.args(args);
        self.isolate(&mut cmd);
        cmd.output().expect("跑 git").status.success()
    }

    /// 当前提交数（含夹具的初始提交）。
    pub fn commit_count(&self) -> i32 {
        self.git_ok(&["rev-list", "--count", "HEAD"])
            .parse()
            .expect("rev-list --count 是数字")
    }

    /// bit 的 AI 配置文件（隔离环境里的 `BIT_CONFIG`）。
    pub fn config_path(&self) -> PathBuf {
        self.dir.path().join("config.toml")
    }

    /// 直接写一份配置文件（重配 / 坏配置用例）。
    pub fn write_config(&self, contents: &str) {
        fs::write(self.config_path(), contents).expect("写配置文件");
    }

    /// 编辑器 stub 的模式：`write` = 每次写入消息；`quit` = 每次都不动文件就退出（vim 的 `:q`）。
    pub fn editor_mode(&self, mode: &str) {
        fs::write(self.stub_dir.join("mode"), mode).expect("写 stub 模式");
    }

    /// stub 要写进消息文件的内容（字节级拷贝，中文不会被任何 shell 的编码转一道）。
    pub fn editor_message(&self, message: &str) {
        fs::write(self.stub_dir.join("message.txt"), message).expect("写 stub 消息");
    }

    /// stub 每次被拉起的记录：第 n 份 = 它当时读到的消息文件内容（即 bit 的种子）。
    pub fn editor_calls(&self) -> Vec<String> {
        editor_calls_in(&self.calls)
    }

    /// 建一条 bit 命令：隔离环境 + 在夹具仓库里跑。
    pub fn bit_command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(BIT);
        cmd.args(args);
        self.isolate(&mut cmd);
        cmd
    }

    /// 非 pty 跑一次 bit：管道当 stdio（`bit` 的 TTY 预检会拦下交互命令），
    /// 带期限等待并在超时后 kill，避免测试挂死。
    pub fn run_bit(&self, args: &[&str]) -> Run {
        let mut cmd = self.bit_command(args);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = cmd.spawn().expect("起 bit");
        wait_collect(child, args)
    }

    /// 隔离环境：清空后整份搬当前进程环境（`conpty` 的坑），再覆盖要钉死的变量。
    fn isolate(&self, cmd: &mut Command) {
        cmd.current_dir(&self.repo);
        cmd.env_clear();
        for (key, value) in std::env::vars_os() {
            // 外层若在 git 钩子 / 特殊环境下跑测试，别把它的仓库指进来
            if matches!(
                key.to_str(),
                Some("GIT_DIR" | "GIT_WORK_TREE" | "GIT_INDEX_FILE" | "GIT_OBJECT_DIRECTORY")
            ) {
                continue;
            }
            cmd.env(key, value);
        }
        cmd.env("HOME", self.dir.path());
        cmd.env("USERPROFILE", self.dir.path());
        cmd.env("XDG_CONFIG_HOME", self.dir.path().join("xdg"));
        // bit 的 AI 配置也锁进沙箱：不设的话 login 会读 / 写穿开发机上的真配置
        cmd.env("BIT_CONFIG", self.config_path());
        for key in [
            "BIT_AI_PROVIDER",
            "BIT_AI_BASE_URL",
            "BIT_AI_MODEL",
            "BIT_AI_API_KEY",
        ] {
            cmd.env_remove(key);
        }
        // 本地 stub 不该被开发机上的代理环境变量绕过去（ureq 默认读代理）
        cmd.env("NO_PROXY", "127.0.0.1,localhost");
        cmd.env("GIT_CONFIG_GLOBAL", self.dir.path().join("gitconfig"));
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        cmd.env("GIT_TERMINAL_PROMPT", "0");
        cmd.env("GIT_PAGER", "cat");
        cmd.env("GIT_EDITOR", stub_exe());
        cmd.env(STUB_DIR_ENV, &self.stub_dir);
        cmd.env("TERM", "xterm-256color");
        cmd.env("LC_ALL", "C");
        cmd.env("LANG", "C");
    }
}

/// 编辑器 stub 的源码：编译成真可执行文件当 `GIT_EDITOR`。
///
/// #12 的实测建议是「指到一个假编辑器可执行文件最稳，别用一整条命令去赌各平台 shell 的转义」，
/// 这里就照做——而且三平台共用同一份实现（.cmd 在 bit 的调用上下文里会「找不到文件」，
/// 见 #13 的评论）。它先把「读到的消息文件」抄进 `calls/<n>.txt`，再按 `mode` 决定写不写消息；
/// 全程字节拷贝，中文不经任何 shell 的编码。数据目录由 `BIT_E2E_STUB_DIR` 指进来。
const STUB_SOURCE: &str = r#"
use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var_os("BIT_E2E_STUB_DIR").expect("夹具没给 BIT_E2E_STUB_DIR"));
    let message = PathBuf::from(env::args_os().nth(1).expect("bit 会把消息文件传进来"));
    let calls = dir.join("calls");
    let n = fs::read_dir(&calls).expect("读 calls").count();
    fs::copy(&message, calls.join(format!("{n}.txt"))).expect("记一次调用");
    let mode = fs::read_to_string(dir.join("mode")).unwrap_or_default();
    if mode.trim() == "quit" {
        return;
    }
    fs::copy(dir.join("message.txt"), &message).expect("写消息");
}
"#;

/// 数据目录（`mode` / `message.txt` / `calls/`）走哪个环境变量告诉 stub。
const STUB_DIR_ENV: &str = "BIT_E2E_STUB_DIR";

/// 编辑器 stub 的可执行文件：本进程内编一次，之后复用（每个测试进程一份，互不打架）。
fn stub_exe() -> &'static Path {
    use std::sync::OnceLock;

    static STUB: OnceLock<PathBuf> = OnceLock::new();
    STUB.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("editor-stub-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("建 stub 源目录");
        let source = dir.join("editor_stub.rs");
        fs::write(&source, STUB_SOURCE).expect("写 stub 源码");
        let exe = dir.join(if cfg!(windows) {
            "editor_stub.exe"
        } else {
            "editor_stub"
        });
        let status = Command::new("rustc")
            .args(["--edition=2021", "-o"])
            .arg(&exe)
            .arg(&source)
            .status()
            .expect("调 rustc 编编辑器 stub（cargo test 环境里应当有它）");
        assert!(status.success(), "编辑器 stub 编译失败");
        exe
    })
}

/// 读一份 stub 记录目录：第 n 份 = stub 第 n 次被拉起时读到的消息文件内容。
fn editor_calls_in(calls: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(calls) else {
        return Vec::new();
    };
    let mut paths: Vec<(u32, PathBuf)> = entries
        .map(|entry| {
            let path = entry.expect("stub 记录项").path();
            let index = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse().ok())
                .expect("stub 记录以序号命名");
            (index, path)
        })
        .collect();
    paths.sort_by_key(|(index, _)| *index);
    paths
        .into_iter()
        .map(|(_, path)| fs::read_to_string(path).expect("读 stub 记录"))
        .collect()
}

/// 带期限地等一个非 pty 子进程，连 stdout / stderr 一起收回。
fn wait_collect(mut child: std::process::Child, args: &[&str]) -> Run {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match child.try_wait().expect("等 bit") {
            Some(status) => {
                let output = child.wait_with_output().expect("收 bit 输出");
                return Run {
                    code: status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                };
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                panic!("bit {args:?} 超时未退（{TIMEOUT:?}）");
            }
            None => std::thread::sleep(POLL),
        }
    }
}

/// 一条跑在 pty 上的 bit：读到的字节全部喂给 `vt100`，断言只看屏幕网格。
pub struct Pty {
    session: OsSession,
    parser: vt100::Parser,
    raw: Vec<u8>,
    calls: PathBuf,
    eof: bool,
}

impl Pty {
    /// 在 ConPTY / pty 上起一条 bit，并把尺寸钉成 80×24。
    pub fn spawn(fixture: &Fixture, args: &[&str]) -> Pty {
        let command = fixture.bit_command(args);
        let mut session = Session::spawn(command).expect("起 pty 会话");
        set_size(&mut session);
        Pty {
            session,
            parser: vt100::Parser::new(ROWS, COLS, 0),
            raw: Vec::new(),
            calls: fixture.calls.clone(),
            eof: false,
        }
    }

    /// 等到屏幕上出现 `needle` 为止；超时 panic，并把屏幕与原始流一起贴出来。
    ///
    /// 出现之后还会等输出静一小会儿再返回：inquire 在提示之间要关掉又重开 raw 模式，
    /// 抢在它换模式的那一瞬间发按键会被控制台丢掉（实测：确认行停在原地、15s 都不动）。
    pub fn expect_screen(&mut self, needle: &str) {
        self.expect_screen_with(needle, false);
    }

    /// 同 [`Pty::expect_screen`]，但先把网格上的自动折行拼回一行再找：
    /// 长文案在 80 列里会断行，断点随路径长度变化，整句断言得先拿掉换行。
    pub fn expect_screen_flat(&mut self, needle: &str) {
        self.expect_screen_with(needle, true);
    }

    fn expect_screen_with(&mut self, needle: &str, flatten: bool) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.drain();
            let screen = self.screen();
            let haystack = if flatten {
                screen.replace('\n', "")
            } else {
                screen
            };
            if haystack.contains(needle) {
                self.settle();
                return;
            }
            if Instant::now() >= deadline {
                self.kill();
                panic!("{TIMEOUT:?} 内屏幕上没等到 {needle:?}\n{}", self.dump());
            }
            std::thread::sleep(POLL);
        }
    }

    /// 等输出安静下来（或到达上限）：`settle()` 之后发按键才稳。
    fn settle(&mut self) {
        let deadline = Instant::now() + SETTLE_LIMIT;
        let mut seen = self.raw.len();
        let mut quiet_since = Instant::now();
        while Instant::now() < deadline {
            self.drain();
            if self.raw.len() != seen {
                seen = self.raw.len();
                quiet_since = Instant::now();
            } else if quiet_since.elapsed() >= SETTLE {
                return;
            }
            std::thread::sleep(POLL);
        }
    }

    /// 往 pty 里塞按键（Enter 用 `\r`，Esc 用 `\x1b`）。
    pub fn send(&mut self, keys: &str) {
        expectrl::Expect::send(&mut self.session, keys).expect("往 pty 写");
    }

    /// 等 bit 退出并给出退出码；超时 panic（先 kill 再报）。
    pub fn exit_code(&mut self) -> i32 {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.drain();
            if let Some(code) = poll_exit(&mut self.session, self.eof) {
                return code;
            }
            if Instant::now() >= deadline {
                self.kill();
                let eof = self.eof;
                panic!(
                    "{TIMEOUT:?} 内 bit 没退出（pty 主端 EOF={eof}）\n{}",
                    self.dump()
                );
            }
            std::thread::sleep(POLL);
        }
    }

    /// 当前屏幕（`vt100` 网格，行尾空格已由解析器裁掉）：读之前先把 pty 上的新输出倒进来。
    pub fn screen(&mut self) -> String {
        self.drain();
        self.parser.screen().contents()
    }

    /// 诊断文本：屏幕 + 原始字节流 + 编辑器 stub 的调用记录。
    fn dump(&mut self) -> String {
        let calls = editor_calls_in(&self.calls);
        format!(
            "屏幕：\n{}\n原始流：\n{}\n编辑器调用（{} 次）：{:?}",
            self.screen(),
            String::from_utf8_lossy(&self.raw),
            calls.len(),
            calls
        )
    }

    /// 把 pty 上现有的字节倒干净：喂解析器、留原始流；读到尽头就记下 EOF。
    fn drain(&mut self) {
        let mut buf = [0u8; 4096];
        loop {
            match self.session.try_read(&mut buf) {
                Ok(0) => {
                    self.eof = true;
                    return;
                }
                Ok(n) => {
                    self.raw.extend_from_slice(&buf[..n]);
                    self.parser.process(&buf[..n]);
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => return,
                // 其余读错误（macOS 上子进程退出后主端就是 EIO）同样意味着这一侧结束了
                Err(_) => {
                    self.eof = true;
                    return;
                }
            }
        }
    }

    /// 断言失败即 kill（#6 的测试卫生）：别把 pty 与子进程漏给后面的用例。
    fn kill(&mut self) {
        #[cfg(windows)]
        let _ = self.session.get_process_mut().exit(1);
        #[cfg(unix)]
        let _ = self.session.get_process_mut().exit(true);
    }
}

/// pty 尺寸：Unix 是 `(cols, rows)`，Windows 的 ConPTY 是 `(x, y)`——差异只关在这里。
#[cfg(unix)]
fn set_size(session: &mut OsSession) {
    session
        .get_process_mut()
        .set_window_size(COLS, ROWS)
        .expect("设 pty 尺寸");
}

#[cfg(windows)]
fn set_size(session: &mut OsSession) {
    session
        .get_process_mut()
        .resize(COLS as i16, ROWS as i16)
        .expect("设 ConPTY 尺寸");
}

/// 退出码：Unix 是 `WaitStatus`，Windows 的 ConPTY 是裸 `u32`（#13 的实测），
/// 平台差异只发生在这一处。
///
/// Unix 侧以 **pty 主端 EOF** 为准：macOS 实测（CI 首跑）里进程明明干完活退出了，
/// `PtyProcess::status()`（`waitpid(WNOHANG)`）却一直报 `StillAlive`，于是 e2e 假设失败。
/// 主端读到尽头就说明子进程那一侧已经关了，这时再用阻塞 `wait()` 收退出码。
#[cfg(unix)]
fn poll_exit(session: &mut OsSession, eof: bool) -> Option<i32> {
    use expectrl::process::unix::WaitStatus;

    match session.get_process_mut().status() {
        Ok(WaitStatus::Exited(_, code)) => return Some(code),
        Ok(WaitStatus::StillAlive) => {}
        Ok(other) => panic!("bit 不是正常退出：{other:?}"),
        Err(_) if !eof => return None,
        Err(error) => panic!("取不到 bit 的退出状态：{error}"),
    }
    if !eof {
        return None;
    }
    match session.get_process_mut().wait() {
        Ok(WaitStatus::Exited(_, code)) => Some(code),
        Ok(other) => panic!("bit 不是正常退出：{other:?}"),
        Err(error) => panic!("wait 拿不到退出码：{error}"),
    }
}

/// 见 Unix 侧的说明：Windows 的 `wait(Some(0))` 就是「还在跑吗」的答案。
#[cfg(windows)]
fn poll_exit(session: &mut OsSession, _eof: bool) -> Option<i32> {
    match session.get_process_mut().wait(Some(0)) {
        Ok(code) => Some(code as i32),
        Err(_) => None,
    }
}

// ---------------------------------------------------------------------------
// AI 供给 stub：login 的 e2e 拿它当「真 HTTP 端点」，全离线
// ---------------------------------------------------------------------------

/// 一条 stub 收到的请求（只取路由需要的两段）。
pub struct AiRequest {
    pub method: String,
    pub path: String,
}

/// stub 的一次应答。
pub struct AiReply {
    pub status: u16,
    pub body: String,
}

impl AiReply {
    /// 200 + JSON 体。
    pub fn ok(body: impl Into<String>) -> AiReply {
        AiReply {
            status: 200,
            body: body.into(),
        }
    }

    /// 指定状态码 + JSON 体（401 / 404 / 429 / 5xx 等）。
    pub fn status(code: u16, body: impl Into<String>) -> AiReply {
        AiReply {
            status: code,
            body: body.into(),
        }
    }
}

/// `GET /models` 的成功体。
pub fn models_body(ids: &[&str]) -> String {
    let entries: Vec<String> = ids.iter().map(|id| format!(r#"{{"id":"{id}"}}"#)).collect();
    format!(r#"{{"data":[{}]}}"#, entries.join(","))
}

/// `POST /chat/completions` 的成功体（验证只要求信封可用）。
pub const CHAT_OK_BODY: &str = r#"{"choices":[{"message":{"content":"ok"}}]}"#;

/// 本地 AI 供给 stub：一个 TcpListener，按闭包应答；Drop 时停服务、收线程。
pub struct AiStub {
    base_url: String,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl AiStub {
    /// 起一个 loopback 上的假供给；`respond` 对每条请求给一次应答。
    pub fn start<F>(respond: F) -> AiStub
    where
        F: Fn(&AiRequest) -> AiReply + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定 stub 端口");
        let port = listener.local_addr().expect("读 stub 地址").port();
        listener.set_nonblocking(true).expect("stub 非阻塞监听");
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&shutdown);
        let handle = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve_stub(stream, &respond),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        AiStub {
            base_url: format!("http://127.0.0.1:{port}"),
            shutdown,
            handle: Some(handle),
        }
    }

    /// 填进 base_url 的地址。
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 一个没有监听者的 loopback 地址（网络错误用例用）。
    pub fn dead_base_url() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("借一个空闲端口");
        let port = listener.local_addr().expect("读借来的地址").port();
        drop(listener);
        format!("http://127.0.0.1:{port}")
    }
}

impl Drop for AiStub {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// 处理一条连接：读完请求 → 问闭包 → 写响应，随即关连接。
fn serve_stub<F>(mut stream: TcpStream, respond: &F)
where
    F: Fn(&AiRequest) -> AiReply,
{
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    // Windows 上 accept 出来的套接字会继承监听端的非阻塞模式，显式改回阻塞
    stream.set_nonblocking(false).ok();
    let Some(request) = read_http_request(&mut stream) else {
        return;
    };
    let reply = respond(&request);
    let reason = match reply.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        reply.body.len(),
        reply.body,
        status = reply.status
    );
    let _ = stream.write_all(response.as_bytes());
}

/// 读一条完整请求（头 + 按 Content-Length 读完请求体），只解析请求行。
fn read_http_request(stream: &mut TcpStream) -> Option<AiRequest> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = match stream.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(_) => return None,
        };
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(head_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&bytes[..head_end]).into_owned();
            if bytes.len() >= head_end + 4 + http_content_length(&head) {
                let line = head.lines().next()?;
                let mut parts = line.split_whitespace();
                return Some(AiRequest {
                    method: parts.next()?.to_string(),
                    path: parts.next()?.to_string(),
                });
            }
        }
    }
    None
}

/// 请求头里的 Content-Length；缺头按 0 算（stub 只收 ureq 的请求）。
fn http_content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}
