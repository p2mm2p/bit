Part of #2 · ticket #6

# 研究 · 交互式 CLI 的 pty 端到端测试选型

> 问题：`bit` 的 e2e 如何驱动真实二进制走交互路径（交互层为 inquire 0.9.4 / crossterm 后端）？
> 选哪个 pty 库？断言什么？三平台 CI（ubuntu + macos + windows）上有什么坑？
>
> 采集日 2026-09-16。事实来自 crates.io API / docs.rs / 上游仓库原始文件的实际抓取（见文末「出处」），
> **未在本机实跑 pty 方案**——本机实跑留给实现阶段，届时若与本文不符，以实测为准。
> 交互库侧的本机一手实测（非 TTY 行为、取消、headless 钩子缺席）见分支 `prototype/interaction-libs`
> 的 `prototype/interaction-libs/FINDINGS.md`，该文 §4.1 的结论「外部无 headless 测试钩子，e2e 只能靠 pty」
> 是本研究的出发点。

---

## 0. 结论摘要

1. **主选 `expectrl 0.9.0`**：唯一同时覆盖 Unix pty 与 Windows ConPTY 的高层库（`expect` / `send` API）。
   风险是单维护者、发版慢 → Cargo.toml 固定 `=0.9.0`，迁移路径是下面的备选。
2. **备选 `portable-pty 0.9.0` + 手写 expect 循环**：完全掌控尺寸 / 生命周期 / 退出码，无单维护者风险；
   代价是约百行自写驱动（`testty` 内部也是这么搭的）。
3. **`rexpect 0.7.1` 不支持 Windows**（Windows Support issue 自 2020-02 起长期 open）——与本项目
   「三平台都跑 e2e」冲突，不选。
4. **屏幕断言用 `vt100`**：`expect` 匹配的是**含 ANSI 转义的原始字节流**；ConPTY 会二次渲染、
   字节流与 Unix 不同，跨平台断言只应打在「vt100 解析后的屏幕网格」上。
5. 关键坑：pty 尺寸必须显式设置（80×24）、Unix 设 `TERM=xterm-256color`、Enter 用 `\r`、
   `GIT_EDITOR` 必须固定为 stub、每用例独立临时仓库、expect 超时 10–15s、断言失败即 kill（防 fd 泄漏）。

---

## 1. 候选对比（2026-09-16 抓取）

| 维度 | expectrl | rexpect | portable-pty |
| --- | --- | --- | --- |
| 最新版本 / 发布 | **0.9.0** / 2026-05-11 | 0.7.1 / 2026-05-14 | 0.9.0 / 2025-02-11 |
| 仓库 | `zhiburt/expectrl`（215★，单维护者） | `rust-cli/rexpect`（392★，组织维护） | `wezterm/wezterm` 的 `pty/`（发版稀疏：0.8.1→0.9.0 约 2 年） |
| 下载量（总 / 近期） | 771k / 304k | 1.98M / 385k | 15.3M / 7.9M |
| Linux / macOS | ✅ pty | ✅ | ✅ forkpty |
| **Windows** | ✅ **ConPTY**（README 原文 “It works on windows.”，后端 `zhiburt/conpty`） | ❌ **仅 Unix**（issue #11 长期 open） | ✅ **ConPTY**（`native_pty_system()` → `win::conpty::ConPtySystem`） |
| API 形态 | 高层：`spawn` / `expect` / `send` / `send_line` | 高层：`spawn` / `exp_string` / `send_line` / `send_control` | 低层 trait：`openpty` / `CommandBuilder` / `try_clone_reader` / `take_writer` |
| 许可证 | MIT | MIT OR Apache-2.0 | MIT |
| 备注 | edition 2021；`async` / `polling` feature，官方文档对 Windows 的 `polling` 有告警 | edition 2024；MSRV 1.85 | `ExitStatus::exit_code()` 跨平台语义清晰 |

观察项（本次不押注）：

- **`testty` 0.15.15**（2026-09-10）：面向 TUI 的「屏幕级断言 + 快照 + 证据报告」框架，理念最贴题；
  但发布仅约半年、77 个版本、总下载 1.7k，API 剧烈变动 → 不作为生产依赖。
- **`trycmd` 1.2.1**：适合**非交互**命令的输出快照；本策略的「命令面只做退出码级断言」已拒绝文案快照，
  故不引入。

---

## 2. 推荐与理由

**主选：`expectrl` 0.9.0（Cargo.toml 固定 `=0.9.0`）。**

- **平台**：Unix pty 与 Windows ConPTY 双支持，正好匹配三平台 CI 矩阵。
- **API 简单度**：`expect(needle)` + `send(...)` 模型与 Don Libes expect / pexpect 同构，
  测试可读性最好、样板最少：`Session::spawn(cmd)` → `set_expect_timeout(Some(...))` →
  `get_process_mut().set_window_size(cols, rows)` → `expect` / `send` → `WaitStatus`。
- **风险对冲**：单维护者、发版节奏慢（2025-09 → 2026-05 才发 0.9）、20 个 open issue →
  固定小版本号；若维护停滞，按备选方案迁移（两者场景同构，迁移面集中在一个测试 helper 里）。

**备选：`portable-pty` 0.9.0 + 手写驱动。**

- 完全掌握 `PtySize`、reader / writer 与子进程句柄；`ExitStatus::exit_code()` 跨平台清晰；
  `Child::kill` 可在断言失败时可靠清理。
- 代价：自实现「读字节 → 匹配提示 → 发按键 → 超时」的小循环（不到 100 行）。

**不选 `rexpect`**：API 与 expectrl 类似且维护更规范，但仅 Unix——无法满足三平台 CI 需求。
（若将来把 e2e 收窄到 ubuntu/macos，可平替。）

---

## 3. 最小可用示例（expectrl）

```rust
// tests/e2e_branch.rs（骨架；提示文案按实际 UI 调整）
use std::process::Command;
use std::time::Duration;

use expectrl::{Expect, Session, WaitStatus};

#[test]
fn bit_branch_creates_branch() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bit"));
    cmd.arg("branch")
        .env("TERM", "xterm-256color")      // Unix 下影响 crossterm 能力探测与配色
        .env("GIT_CONFIG_NOSYSTEM", "1")    // 隔离本机 git 配置
        .env("GIT_TERMINAL_PROMPT", "0")    // 关闭凭据提示
        .env("GIT_PAGER", "cat");           // 关闭 pager

    let mut p = Session::spawn(cmd)?;       // Unix: forkpty；Windows: ConPTY
    p.set_expect_timeout(Some(Duration::from_secs(15)));
    p.get_process_mut().set_window_size(80, 24)?; // (cols, rows)，不设会读到 0×0

    p.expect("选择分支类型")?;
    p.send("\x1b[B")?;                      // ↓（CSI 形式；crossterm 也接受 SS3 \x1bOB）
    p.send("\r")?;                          // Enter（raw 模式下 CR 最稳）
    p.expect("请输入分支名")?;
    p.send("feature/e2e-demo")?;
    p.send("\r")?;
    p.expect(expectrl::Eof)?;

    match p.get_process_mut().wait()? {
        WaitStatus::Exited(_, 0) => Ok(()),
        other => panic!("unexpected exit status: {other:?}"),
    }
}
```

要点：

- `Session::spawn(Command)`、`set_expect_timeout(Option<Duration>)`、`get_process_mut()` 是
  `expectrl::session::Session` 的方法；`expect` / `send` 来自 `Expect` trait。
- `get_process_mut()` 返回 `UnixProcess`（Deref 到 `ptyprocess::PtyProcess`），其上还有
  `is_alive()`、`exit(force)`。
- 退出码：Unix 为 `WaitStatus::Exited(Pid, i32)`，注意区别于 `Signaled`；**Windows 后端形态可能不同**——
  实现时把退出码断言收敛进一个小 helper，平台差异只发生在一处。

---

## 4. 屏幕断言：`vt100`

`expect` 在原始字节流上匹配（含 ANSI 转义），ConPTY 下字节流与 Unix 不一致；要断言「渲染后的画面」
必须接终端解析器：

```rust
let mut parser = vt100::Parser::new(24, 80, 0); // rows, cols, scrollback
parser.process(&raw_bytes);                     // 累积喂入 pty 输出
let screen = parser.screen();
assert!(screen.contents().contains("选择分支类型"));
let (row, col) = screen.cursor_position();      // 光标位置也可断言
```

- `vt100::Screen`：`contents()`、`rows(start, width)`、`contents_between(...)`、`cell(row, col)`、
  `cursor_position()`、`alternate_screen()` 等。
- `Cell` 可断言前景 / 背景色与加粗、下划线等属性（inquire 的「绿色标注」类断言可着落在这里）。
- 本项目的断言形态建议：`expect(关键提示文本)`（等待）→ 断言 `screen.contents()` 包含目标文本，
  避免对转义序列做字节级匹配。

---

## 5. 已知坑清单（本项目 crossterm + inquire + git 场景）

**CI 稳定性**

- 不用固定 `sleep`；一切等待都走「等到提示出现」的 expect，超时给足（10–15s，runner 冷启动慢）。
- 每个用例独占 pty + 独立临时 git 仓库；隔离 `HOME` / `GIT_CONFIG_GLOBAL` / `GIT_CONFIG_NOSYSTEM`，
  避免读进本机全局配置。
- 断言失败即 panic 会泄漏子进程与 pty fd：用 guard / `Child::kill` / `exit(force)` 兜底回收，
  否则 macOS 上反复跑会耗尽 `/dev/ttys*`。

**pty 尺寸 / TERM**

- 必须显式设置 pty 尺寸（`set_window_size(80, 24)` 或 `PtySize { rows, cols, .. }`）；
  尺寸为 0 时 crossterm / inquire 的换行与高亮会错乱。
- Unix 设 `TERM=xterm-256color`；`TERM=dumb` 或未设置会让颜色与能力探测退化 → 断言漂移。
- Windows ConPTY 下 `TERM` 基本无意义（crossterm 走 Win32 控制台 API）。

**Windows 与 Unix 差异**

- ConPTY 会二次渲染终端状态：**不要断言原始字节**，只断言 vt100 解析后的网格。
- Enter 用 `\r`（CR）最稳；`send_line` 的 `\n` 在部分实现下可能不被当作 Enter。
- 方向键 CSI（`\x1b[A/B/C/D`）与 SS3（`\x1bOA`…）两种编码都存在；自定义匹配器要两种都处理。
- `expectrl` 的 `polling` feature 官方警告 Windows 慎用（会起线程）——同步 `expect` 不受影响。

**超时与清理 / bit 特有**

- `bit commit` 会拉起编辑器：测试里必须把 `GIT_EDITOR` 固定为确定性脚本（写文件后退出），
  否则挂死等待真实编辑器；「空 subject → 重开编辑器」的回环用例也用同一 stub 实现。
- 防 pager：`GIT_PAGER=cat`（或命令带 `--no-pager`）。
- 关闭交互式凭据提示：`GIT_TERMINAL_PROMPT=0`。
- 每用例用 `tempfile` 建独立仓库（`git init`，注意新版 git 默认分支名可能非 `master`），结束即清理。
- 跨平台退出码断言收敛到一个 helper（见 §3 末尾）。

---

## 出处（一手，2026-09-16 抓取）

1. `expectrl` 元数据（版本 / 日期 / 下载） — <https://crates.io/api/v1/crates/expectrl>；
   仓库活跃度 — <https://api.github.com/repos/zhiburt/expectrl>；
   README（“It works on windows.”） — <https://raw.githubusercontent.com/zhiburt/expectrl/main/README.md>
2. `expectrl` API — <https://docs.rs/expectrl/latest/expectrl/>；
   `Session` — <https://docs.rs/expectrl/latest/expectrl/session/struct.Session.html>；
   `UnixProcess` — <https://docs.rs/expectrl/latest/expectrl/process/unix/struct.UnixProcess.html>；
   `WaitStatus` — <https://docs.rs/expectrl/latest/expectrl/process/unix/enum.WaitStatus.html>
3. `expectrl` 的 Windows 后端（ConPTY 抽象） — <https://github.com/zhiburt/conpty>
4. `rexpect` 元数据 / 分类（仅 Unix） — <https://crates.io/api/v1/crates/rexpect>；
   仓库 — <https://api.github.com/repos/rust-cli/rexpect>；
   Windows Support issue #11 — <https://github.com/rust-cli/rexpect/issues/11>
5. `portable-pty` 元数据 — <https://crates.io/api/v1/crates/portable-pty>；
   API — <https://docs.rs/portable-pty/latest/portable_pty/>；
   ConPTY 实现（`pty/src/lib.rs`） — <https://raw.githubusercontent.com/wezterm/wezterm/main/pty/src/lib.rs>
6. `vt100` 元数据 — <https://crates.io/api/v1/crates/vt100>；
   API — <https://docs.rs/vt100/latest/vt100/>；
   `Screen` — <https://docs.rs/vt100/latest/vt100/struct.Screen.html>
7. `testty`（观察项） — <https://crates.io/api/v1/crates/testty>；<https://docs.rs/testty/latest/testty/>
8. `trycmd`（非交互快照，本次不引入） — <https://crates.io/api/v1/crates/trycmd>
9. `inquire` 0.9.4 元数据（后端为 crossterm，默认 feature） — <https://crates.io/api/v1/crates/inquire>
10. 交互库本机一手实测（非 TTY、取消、headless 钩子缺席） — 分支 `prototype/interaction-libs`，
    `prototype/interaction-libs/FINDINGS.md`

---

## 6. 实现期实测修订（2026-09-16，随 [实现 · pty e2e 与三平台 CI](https://github.com/p2mm2p/bit/issues/13) 落地）

本文开头写着「未在本机实跑 pty 方案，本机实跑留给实现阶段，届时若与本文不符，以实测为准」。
实跑之后的修订，以及夹具最终形态（`tests/common/mod.rs`）。

1. **ConPTY 给子进程的标准句柄是「驱动标准句柄的副本」，不是那个伪控制台的。** 把探针跑在 pty 里
   让它报告自己：驱动被管道喂时，子进程 stdout 的 `GetFileType` 仍是 `FILE_TYPE_PIPE`、stdin 是
   NUL 设备、`GetConsoleWindow()` 为空——驱动自己都没有控制台。bit 的 TTY 预检因此会把自己拦下，
   pty 里的输入也送不到它手里。
2. **要一份真控制台：`Start-Process` 好使，`cmd /C start` 不好使。** 后者把 cmd 自己的管道句柄
   一并传给子进程（实测：子进程照旧 `is_terminal=false`）；`Start-Process -WindowStyle Hidden`
   走 ShellExecute，子进程三个流都是控制台。夹具因此是：驱动没有控制台标准流时，用 PowerShell
   把同一份用例重起一遍（子进程输出躺在隐藏控制台里，失败信息经报告文件回传）。
3. **编辑器 stub 用「假可执行文件」，不要用脚本。** 第一版是 `.cmd`：同一个 .cmd 从交互 shell
   手跑没问题，但在 bit 的调用上下文里 cmd 一直报「系统找不到指定的文件。」，bit 于是无限重开
   编辑器（#13 复审时留痕）。这与 #12 备注里「别用带引号/管道的一整条命令去赌各平台 shell 的
   转义、用假编辑器可执行文件最稳」一致。最终 stub 是内嵌在夹具里的 Rust 源码，测试进程里用
   `rustc` 编一次、三平台共用一份。
4. **发按键前要等界面静下来。** inquire 在提示之间关掉又重开 raw 模式，抢在换模式的那一刻发
   `\r` 会被控制台丢掉（实测：确认行停在原地、15s 都不动）。夹具的 `expect_screen` 因此是
   「等到屏幕出现目标文本，再等 120ms 没有新输出」才返回。
5. **读取与断言**：夹具不用 `expect`（见 §4，ConPTY 会二次渲染、字节流不可作断言），而是
   `Session::try_read` 轮询、字节全部喂给 `vt100`，断言打在 `screen().contents()` 上；
   pty 尺寸显式 80×24、Enter 用 `\r`、Esc 用 `\x1b`、Unix 设 `TERM=xterm-256color`。
6. **退出码**：Windows 走 `conpty::Process::wait(Some(ms)) -> u32`，Unix 走 `PtyProcess::status()`
   的 `WaitStatus`（非破坏性轮询；`is_alive()` 会顺手 reap，之后就取不到退出码了）。平台差异
   关在夹具的两个小函数里。
7. **环境隔离**按 #6 的清单钉死（`HOME` / `USERPROFILE` / `GIT_CONFIG_GLOBAL` /
   `GIT_CONFIG_NOSYSTEM` / `GIT_PAGER` / `GIT_TERMINAL_PROMPT` / `LC_ALL`），且**不用 `.env()`
   单加**——conpty 只拼显式 set 过的变量，那样会把 `PATH` 一起丢掉（#13 评论的第 2 条坑）；
   夹具是 `env_clear()` 后整份搬当前环境、再覆盖要隔离的那几个。

