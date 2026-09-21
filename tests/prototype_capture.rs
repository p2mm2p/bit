//! 抛弃型原型夹具：驱动 `examples/login_stub.rs` 走一遍每条路径，把每个提示的
//! vt100 屏幕网格落盘到 `docs/prototype/26-grids/`。
//!
//! 它只服务 [原型 · bit login 向导交互与文案](https://github.com/p2mm2p/bit/issues/26)
//! 的走查，随原型分支一起丢弃，不进 main。跑法：
//!
//! ```text
//! cargo build --example login_stub
//! cargo test --test prototype_capture -- --nocapture
//! ```

mod common;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use expectrl::Session;
use expectrl::session::OsSession;

const COLS: u16 = 80;
const ROWS: u16 = 24;
const TIMEOUT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(10);
/// 提示出现后再等这么久没有新输出，才认为界面稳住了（抄 tests/common 的做法）。
const SETTLE: Duration = Duration::from_millis(150);
const SETTLE_LIMIT: Duration = Duration::from_secs(3);

#[test]
fn capture_login_wizard_grids() {
    if common::ensure_console("capture_login_wizard_grids") {
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/prototype/26-grids");
    fs::create_dir_all(&dir).expect("建网格目录");

    happy(&dir);
    fallback(&dir);
    custom(&dir);
    local(&dir);
    openrouter_filter(&dir);
    auth_fail(&dir);
    network_fail(&dir);
    model_fail(&dir);
    cancel(&dir);
    reconfig(&dir);
    variants(&dir);

    println!("网格已写入 {}", dir.display());
}

fn happy(dir: &Path) {
    let mut grid = Grid::spawn(dir, "01-happy", &[]);
    grid.expect("? 提供商");
    grid.snap("provider-menu");
    grid.send("\r");

    grid.expect("? API Key");
    grid.snap("key-masked");
    grid.send("sk-proj-demo-123\r");

    grid.expect("正在获取模型列表");
    grid.expect("deepseek-chat");
    grid.snap("model-menu");
    grid.send("\r");

    grid.expect("已保存 AI 供给");
    grid.snap("summary");
    assert_eq!(grid.exit_code(), 0, "主路径应当成功");
}

fn fallback(dir: &Path) {
    let mut grid = Grid::spawn(dir, "02-fallback", &["--provider=zhipu"]);
    grid.expect("? 提供商");
    grid.send("\r");

    grid.expect("? API Key");
    grid.send("sk-demo-zhipu\r");

    grid.expect("正在获取模型列表");
    grid.expect("未能获取模型列表");
    grid.snap("model-manual");
    grid.send("GLM-5.3\r");

    grid.expect("已保存 AI 供给");
    grid.snap("summary");
    assert_eq!(grid.exit_code(), 0);
}

fn custom(dir: &Path) {
    let mut grid = Grid::spawn(dir, "03-custom", &["--provider=custom"]);
    grid.expect("? 提供商");
    grid.send("\r");

    grid.expect("? base_url");
    grid.snap("base-url");
    grid.send("https://api.example.com/v1\r");

    grid.expect("? API Key");
    grid.send("sk-demo-custom\r");

    grid.expect("未能获取模型列表，请手动输入模型名");
    grid.snap("model-manual-custom");
    grid.send("my-model\r");

    grid.expect("已保存 AI 供给");
    assert_eq!(grid.exit_code(), 0);
}

fn local(dir: &Path) {
    let mut grid = Grid::spawn(dir, "04-local", &["--provider=ollama"]);
    grid.expect("? 提供商");
    grid.send("\r");

    grid.expect("本地 Ollama 无需密钥");
    grid.snap("key-local");
    grid.send("\r");

    grid.expect("正在获取模型列表");
    grid.expect("qwen3:8b");
    grid.snap("model-menu-local");
    grid.send("\r");

    grid.expect("已保存 AI 供给");
    assert_eq!(grid.exit_code(), 0);
}

fn openrouter_filter(dir: &Path) {
    let mut grid = Grid::spawn(dir, "05-openrouter", &["--provider=openrouter"]);
    grid.expect("? 提供商");
    grid.send("\r");

    grid.expect("? API Key");
    grid.send("sk-or-demo\r");

    grid.expect("正在获取模型列表");
    grid.expect("anthropic/claude-4.6-sonnet");
    grid.send("glm");
    // 等筛选后的屏幕稳住再抓（让渲染跟上输入）。
    grid.expect("z-ai/glm-5.3");
    grid.snap("model-menu-filtered");

    grid.send("\x1b");
    grid.expect("已取消：未做任何改动。");
    assert_eq!(grid.exit_code(), 1);
}

fn auth_fail(dir: &Path) {
    let mut grid = Grid::spawn(dir, "06-auth", &["--provider=deepseek", "--fail=auth"]);
    grid.expect("? 提供商");
    grid.send("\r");

    grid.expect("? API Key");
    grid.send("sk-bad-key\r");

    grid.expect("正在获取模型列表");
    grid.expect("deepseek-chat");
    grid.send("\r");

    grid.expect("认证失败（401）");
    grid.pause();
    grid.snap("auth-401");
    // 401 之后回到 key 输入：等界面稳住再按键（inquire 重开 raw 模式的空隙会丢键）。
    grid.expect("? API Key");
    grid.send("sk-good-key\r");

    grid.expect("正在获取模型列表");
    grid.expect("deepseek-chat");
    grid.send("\r");

    grid.expect("已保存 AI 供给");
    assert_eq!(grid.exit_code(), 0, "重输 key 之后应当成功");
}

fn network_fail(dir: &Path) {
    let mut grid = Grid::spawn(dir, "07-network", &["--provider=deepseek", "--fail=network"]);
    grid.expect("? 提供商");
    grid.send("\r");

    grid.expect("? API Key");
    grid.send("sk-demo\r");

    grid.expect("正在获取模型列表");
    grid.expect("deepseek-chat");
    grid.send("\r");

    grid.expect("错误：连不上");
    grid.snap("network-error");
    assert_eq!(grid.exit_code(), 1);
}

fn model_fail(dir: &Path) {
    let mut grid = Grid::spawn(dir, "08-model", &["--provider=deepseek", "--fail=model"]);
    grid.expect("? 提供商");
    grid.send("\r");

    grid.expect("? API Key");
    grid.send("sk-demo\r");

    grid.expect("正在获取模型列表");
    grid.expect("deepseek-chat");
    grid.send("\r");

    grid.expect("模型不可用（404）");
    grid.pause();
    grid.snap("model-404");
    grid.expect("deepseek-chat");
    grid.send("\r");

    grid.expect("已保存 AI 供给");
    assert_eq!(grid.exit_code(), 0, "换模型之后应当成功");
}

fn cancel(dir: &Path) {
    let mut grid = Grid::spawn(dir, "09-cancel", &[]);
    grid.expect("? 提供商");
    grid.send("\x1b");

    grid.expect("已取消：未做任何改动。");
    grid.snap("esc-cancel");
    assert_eq!(grid.exit_code(), 1);
}

fn reconfig(dir: &Path) {
    let mut grid = Grid::spawn(dir, "10-reconfig", &["--reconfig"]);
    grid.expect("当前配置：DeepSeek / deepseek-v4-pro");
    grid.expect("? 提供商");
    grid.snap("provider-menu-reconfig");
    grid.send("\r");

    grid.expect("已配置");
    grid.snap("key-keep");
    grid.send("\r");

    grid.expect("正在获取模型列表");
    grid.expect("? 模型");
    grid.snap("model-menu-current");
    grid.send("\r");

    grid.expect("已保存 AI 供给");
    assert_eq!(grid.exit_code(), 0);
}

fn variants(dir: &Path) {
    let mut plain = Grid::spawn(dir, "11-plain", &["--variant=plain"]);
    plain.expect("? 提供商");
    plain.snap("provider-menu-plain");
    plain.send("\x1b");
    plain.expect("已取消：未做任何改动。");
    assert_eq!(plain.exit_code(), 1);

    let mut quiet = Grid::spawn(
        dir,
        "12-quiet",
        &["--provider=zhipu", "--variant=quiet"],
    );
    quiet.expect("? 提供商");
    quiet.send("\r");
    quiet.expect("? API Key");
    quiet.send("sk-demo\r");
    quiet.expect("正在获取模型列表");
    quiet.expect("请手动输入模型名");
    quiet.snap("model-manual-quiet");
    quiet.send("\x1b");
    quiet.expect("已取消：未做任何改动。");
    assert_eq!(quiet.exit_code(), 1);
}

// ---------------------------------------------------------------------------
// pty 驱动（缩自 tests/common/mod.rs，只保留抓屏要用的部分）
// ---------------------------------------------------------------------------

struct Grid {
    session: OsSession,
    parser: vt100::Parser,
    raw: Vec<u8>,
    dir: PathBuf,
    scenario: &'static str,
    step: u32,
    eof: bool,
}

impl Grid {
    fn spawn(dir: &Path, scenario: &'static str, args: &[&str]) -> Grid {
        let exe = stub_exe();
        assert!(
            exe.is_file(),
            "先 cargo build --example login_stub；找的是 {}",
            exe.display()
        );
        let mut command = Command::new(exe);
        // conpty 只拼显式 set 过的变量（tests/common 的实测坑），整份搬当前环境。
        for (key, value) in std::env::vars_os() {
            command.env(key, value);
        }
        command.args(args).env("TERM", "xterm-256color");
        let mut session = Session::spawn(command).expect("起 pty 会话");
        set_size(&mut session);
        Grid {
            session,
            parser: vt100::Parser::new(ROWS, COLS, 0),
            raw: Vec::new(),
            dir: dir.to_path_buf(),
            scenario,
            step: 0,
            eof: false,
        }
    }

    fn expect(&mut self, needle: &str) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.drain();
            if self.screen().contains(needle) {
                self.settle();
                return;
            }
            if Instant::now() >= deadline {
                let screen = self.screen();
                self.kill();
                panic!("{TIMEOUT:?} 内屏幕上没等到 {needle:?}\n{screen}");
            }
            std::thread::sleep(POLL);
        }
    }

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

    fn send(&mut self, keys: &str) {
        expectrl::Expect::send(&mut self.session, keys).expect("往 pty 写");
    }

    /// 让错误文案与下一个提示都渲染完，再抓屏或按键。
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(400));
        self.drain();
    }

    /// 把当前屏幕写进 `docs/prototype/26-grids/`，文件名里的序号即走查顺序。
    fn snap(&mut self, name: &str) {
        self.step += 1;
        let path = self
            .dir
            .join(format!("{:02}-{}-{name}.txt", self.step, self.scenario));
        let screen = self.screen();
        fs::write(&path, format!("{screen}\n")).expect("写网格");
    }

    fn screen(&mut self) -> String {
        self.drain();
        self.parser.screen().contents()
    }

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
                Err(_) => {
                    self.eof = true;
                    return;
                }
            }
        }
    }

    fn exit_code(&mut self) -> i32 {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.drain();
            if let Some(code) = poll_exit(&mut self.session, self.eof) {
                return code;
            }
            if Instant::now() >= deadline {
                let screen = self.screen();
                self.kill();
                panic!("{TIMEOUT:?} 内没退出\n{screen}");
            }
            std::thread::sleep(POLL);
        }
    }

    fn kill(&mut self) {
        #[cfg(windows)]
        let _ = self.session.get_process_mut().exit(1);
        #[cfg(unix)]
        let _ = self.session.get_process_mut().exit(true);
    }
}

fn stub_exe() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("examples")
        .join(format!("login_stub{}", std::env::consts::EXE_SUFFIX))
}

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

#[cfg(unix)]
fn poll_exit(session: &mut OsSession, eof: bool) -> Option<i32> {
    use expectrl::process::unix::WaitStatus;

    match session.get_process_mut().status() {
        Ok(WaitStatus::Exited(_, code)) => return Some(code),
        Ok(WaitStatus::StillAlive) => {}
        Ok(other) => panic!("不是正常退出：{other:?}"),
        Err(_) if !eof => return None,
        Err(error) => panic!("取不到退出状态：{error}"),
    }
    if !eof {
        return None;
    }
    match session.get_process_mut().wait() {
        Ok(WaitStatus::Exited(_, code)) => Some(code),
        Ok(other) => panic!("不是正常退出：{other:?}"),
        Err(error) => panic!("wait 拿不到退出码：{error}"),
    }
}

#[cfg(windows)]
fn poll_exit(session: &mut OsSession, _eof: bool) -> Option<i32> {
    match session.get_process_mut().wait(Some(0)) {
        Ok(code) => Some(code as i32),
        Err(_) => None,
    }
}
