# bit

两个交互命令的 git 包装，外加一个 AI 供给向导。

- `bit branch` —— 选类型 → 输名字 → 确认，生成规范分支名，交给 `git switch -c`。
- `bit commit` —— 选类型 / scope / breaking，打开编辑器补 subject，交给 `git commit`。
- `bit login` —— 选提供商 / 填密钥 → 验证连通性，写入本机 AI 供给配置。

bit 只做「合规」这一层：分支名照 Conventional Branch，提交消息照 Conventional Commits。其余一切按 git 的原样语义走——编辑器问 `git var GIT_EDITOR`，输出与退出码原样透传，不翻译、不包装。三个命令之外**不做任何透传**：要跑别的 git 命令，请直接用 git。

配置过 AI 供给后，`bit branch` 的描述可直接写中文（自动翻译为英文），`bit commit --gen` 读暂存 diff 生成提交消息；没配置时 bit 是纯离线工具，v0.1 行为零回归。

## 安装

```sh
cargo install --path .
```

需要 Rust 1.98 及以上（edition 2024）。

## bit branch

选类型 → 输名字 → 确认，然后调用 `git switch -c <名字>`。

类型菜单 7 项：`feature`、`fix`、`hotfix`、`release`、`chore`，以及扩展类型 `docs`、`test`（超出 Conventional Branch 内置类型，菜单里已标注；规范里的别名 `feat/`、`bugfix/` 不在清单内，菜单给的是正名）。

名字输入：

- **静默规范化**：trim → 全小写 → 空格/下划线转 `-` → 折叠连续 `-`/`.` → 去首尾分隔符；输入 `Add OAuth Login` 得到 `add-oauth-login`。
- **同类型前缀剥离**：`feature/add-login` 与 `add-login` 等价；带了其它类型前缀则原地报错重输。
- **规范级校验由 bit 承担**：白名单（`a–z 0–9 - .`）之外的字符原地报错重输——不静默丢弃，用户得知道自己的输入被怎么了。成品因此比 git 能接受的名字更严；未配置 AI 供给时中文照旧被拒，报错附带 `bit login` 指路，配置后走「描述翻译」。
- 发生过规范化或前缀剥离时，确认步骤回显 `由 "<原文>" 规范化`。

配置过 AI 供给（`bit login`）后，规范化与前缀剥离之后仍含非 ASCII 的描述先走一次**描述翻译**：

- 触发只凭非 ASCII：纯 ASCII 的非法输入（`add login!`）仍按 v0.1 原地报错，不静默修补。
- 模型只产出描述段；类型前缀组装、白名单校验、最终名仍由 bit 完成，译文清理后仍不合法即判失败。
- 确认门回显 `由 "<原文>" 翻译为 "<最终描述段>"`；选「否」回到名字输入、预填**原文**。
- 失败（认证 / 限流 / 网络 / 服务端 / 模型 / 译文不可用 / 配置错误）一律 `翻译失败：<原因>` 并回到名字输入，不自动重试。

创建前有一次确认（默认「是」）；选「否」回到名字输入并预填原文。重名、非仓库、detached HEAD 等 git 自己的语义不预检，原样交给 git——退出码也一样。

## bit commit

先预检暂存内容，再选类型 → 填 scope（可留空）→ 答 breaking → 编辑器补 subject，最后提交。

类型菜单 11 项（commitlint `type-enum` 全集）：`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`。

会调用的 git 命令：

```
git diff --cached --quiet                                          # 预检：无暂存内容在进交互前拦下
git var GIT_EDITOR                                                 # 编辑器是谁，一律问 git
git rev-parse --path-format=absolute --git-path COMMIT_EDITMSG     # 种子写进 git 自己的消息草稿
git stripspace --strip-comments                                    # 「git 将保存的消息」由 git 自己算
git commit -F <消息文件> --cleanup=strip                            # 提交
```

- 编辑器缓冲区预填 `type(scope)!: `（scope 留空则省略括号），下面一行注释提示；注释随 `--cleanup=strip` 剥掉，不进历史。
- 校验只做规范 MUST 级的一件事：**描述非空**。改动过但描述仍空，会带着你上次保存的原文重开编辑器并说明原因；**什么都没改就退出编辑器 = 放弃提交**（`已取消：`、退出码 1）——回环有界，不在空描述上打转。
- scope 允许中文，但不能含 `(` `)` `:` 或换行。
- 引用 issue 用 footer（`Refs: #123`）；行首的 `#` 会被 git 的注释清理剥掉。

加 `--gen`：不进菜单，读暂存 diff 交给 AI 生成 draft，终端展示、确认后提交；选「否」走编辑器复核（预填 draft，「未改动退出」照旧适用）。会额外调用 `git diff --cached --numstat -z`（文件清单与增删行数）、`git diff --cached --no-color --no-ext-diff --no-textconv --unified=3`（正文）与 `git branch --show-current`（上下文）。

- 供给预检在读 diff 之前：没配置就报 `错误：未配置 AI 供给 —— 请先跑 bit login。`、退出码 1，暂存内容不进内存、不发任何请求。
- 只读暂存区，不读工作区与历史；上下文只附当前分支名。
- 送模型前先过滤：锁文件、`*.min.js` / `*.min.css` / `*.map`、`dist/**`、`target/**` 与二进制只进 stat；正文按 24,000 字符预算，超出走两段式（逐文件中文摘要 → 汇总生成，批数上限 4、总调用 ≤ 5）。
- AI 只出字段；type 不在 11 类内、scope 不合法、subject 为空时给提示并带 draft 进编辑器复核（不开确认门）。
- 生成失败（认证 / 限流 / 网络 / 服务端 / 模型 / 响应不可解析 / 分段摘要失败 / 变更过大）一律 `错误：生成失败：<原因>`、退出码 1；不自动重试、不退回手写、不跳过确认。

## bit login

选提供商 →（自定义）`base_url` → 掩码输 `API Key` → 试拉 `/models`（成功即菜单选模型，失败静默回退手输）→ 一次连通性验证 → 写配置。

- 预置 9 项：DeepSeek、Kimi（Moonshot）、智谱 GLM、通义千问、OpenAI、OpenRouter、SiliconFlow、Ollama（本地，免密钥）、自定义 OpenAI 兼容。
- 失败去向：401 回 key 输入、404 回模型输入、网络 / 429 / 5xx 报错退出 1；不自动重试。
- 重跑即重配：当前值作默认，顶部显示 `当前配置：<显示名> / <模型>`，key 留空保持不变。
- Esc 取消（`已取消：未做任何改动。`、退出码 1），不写任何配置；`bit login` 不是账号登录——bit 不注册、不登录、不绑定账号，只写本机文件。

## AI 配置

`bit login` 写出的「AI 供给」是一个扁平四键、明文 TOML（可手工编辑）：

```toml
# bit 的 AI 供给 —— 由 bit login 生成，可手工编辑。
# 密钥为明文，请勿把本文件提交进仓库。
provider = "deepseek"
base_url = "https://api.deepseek.com"
model = "deepseek-v4-pro"
api_key = "sk-…"
```

- 路径：Windows `%APPDATA%\bit\config.toml`；Unix `$XDG_CONFIG_HOME/bit/config.toml`，兜底 `~/.config/bit/config.toml`；`BIT_CONFIG` 覆盖整条路径。
- 优先序：`BIT_AI_PROVIDER` / `BIT_AI_BASE_URL` / `BIT_AI_API_KEY` / `BIT_AI_MODEL` 逐字段覆盖文件；纯环境变量也能组成完整配置。
- 键名固定，未知键报错；`provider` 可缺省（按 `custom` 处理）。文件坏了不会被环境变量掩盖。
- 写入权限：Unix 0600、原子替换；Windows 用 `%APPDATA%` 默认 ACL、不额外加固。密钥明文——别提交进仓库。
- 读取是惰性的，三种结果：**未配置**（AI 能力不存在，v0.1 两条路径零回归）、**配置错误**（文件在但坏；用到 AI 能力即报错、指路 `bit login`）、**可用**。坏文件拦不住不带 `--gen` 的 `bit commit` 与纯 ASCII 的 `bit branch`。
- 清配置 = 删文件；重跑 `bit login` 即重配。

## 帮助与版本

```sh
bit --help      # 完整帮助
bit -V          # bit 0.2.0
bit branch -h   # 单个命令那一节（含「会调用哪条 git 命令」）
bit login -h    # 同上（含会写入什么）
```

三个命令都是交互式的，不接受参数；`--gen` 是唯一选项，只认 `bit commit --gen` 一种拼法。`bit branch foo`、`bit commit -m x`、`bit login foo` 一律报错并给出直路（`要直接建分支请用 git switch -c <名字>` / `要直接提交请用 git commit` / `要配置 AI 供给请直接运行 bit login`）。

## 退出码

| 码 | 情形 | 流向 |
| --- | --- | --- |
| 0 | 成功；`-h` / `--help` / `-V` / `--version` | stdout（命令成功时输出的是 git 的） |
| 2 | 用法层失败：无参数、未知命令、未知选项、多余参数（含 `--gen` 的多余参数） | stderr |
| 1 | 运行期失败：Esc 取消、非 TTY、无暂存内容、编辑器非 0 退出、未改动退出（放弃提交）、未配置 / 配置错误、生成失败 | stderr |
| 128 等 | git 自身的失败原样透传（重名分支 = 128） | git 的 stderr |
| 130 | Ctrl-C（信号语义），bit 不加文案 | — |

记忆点：**2 = 你还没让 bit 开始做事；1 = bit 开始做事后失败或被取消；128+ = git 说的。**

翻译失败落回名字输入、重开一次输入循环，不是退出点，故不入表。

bit 自己说的话全是中文（`错误：` / `已取消：`、全角标点），git 的原文一字不改——语言本身就是来源标记。

## 边界与出路

- **不做透传**：`bit add .` 报错、不转发；其它 git 命令请直接用 git。
- **不做脚本化路径**：交互需要 TTY（非 TTY 直接拒绝、退出码 1），`bit login` 同样；脚本里请用 git。
- **只服务本机**：提交风格档位（plain / subject+body 等）与发布分发不在范围内。
- **联网只在显式配置之后**：不内置任何默认端点、不做遥测；没跑过 `bit login` 时 bit 是纯离线工具。数据只发往自己配的那个端点——描述翻译发「规范化 + 前缀剥离后的描述段」（附所选类型），`--gen` 发过滤、截断后的暂存 diff + stat + 当前分支名；v0.2 不脱敏。
- **不做 OS keyring**：密钥明文落在配置文件里；v0.2 用明文换零平台依赖，keyring 不做。

## 文档

- [`CONTEXT.md`](./CONTEXT.md) —— 术语表：分支类型、静默规范化、命令面、AI 供给……的规范叫法
- [`docs/adr/`](./docs/adr) —— 四个关键决策：bit 自拉编辑器并自校验、bit 承担分支名规范校验、命令面自成一格、AI 供给外置且显式授权
- [`docs/research/`](./docs/research) —— 研究笔记：Conventional Branch、Conventional Commits、pty e2e 选型、OpenAI 兼容供给面
- [`AGENTS.md`](./AGENTS.md) —— 面向 agent 的仓库约定

## 开发

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test        # 单测 + pty e2e（驱动真实二进制跑交互路径）
```

CI（[`.github/workflows/ci.yml`](./.github/workflows/ci.yml)）在 ubuntu / macos / windows 三平台跑同样三道门。
