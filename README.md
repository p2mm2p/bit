# bit

两个交互命令的 git 包装。

- `bit branch` —— 选类型 → 输名字 → 确认，生成规范分支名，交给 `git switch -c`。
- `bit commit` —— 选类型 / scope / breaking，打开编辑器补 subject，交给 `git commit`。

bit 只做「合规」这一层：分支名照 Conventional Branch，提交消息照 Conventional Commits。其余一切按 git 的原样语义走——编辑器问 `git var GIT_EDITOR`，输出与退出码原样透传，不翻译、不包装。两个命令之外**不做任何透传**：要跑别的 git 命令，请直接用 git。

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
- **规范级校验由 bit 承担**：白名单（`a–z 0–9 - .`）之外的字符原地报错重输（含中文）——不静默丢弃，用户得知道自己的输入被怎么了。成品因此比 git 能接受的名字更严。
- 发生过规范化或前缀剥离时，确认步骤回显 `由 "<原文>" 规范化`。

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
- 校验只做规范 MUST 级的一件事：**描述非空**。什么都没改就退出编辑器，会带着你上次保存的原文重开编辑器。
- scope 允许中文，但不能含 `(` `)` `:` 或换行。
- 引用 issue 用 footer（`Refs: #123`）；行首的 `#` 会被 git 的注释清理剥掉。

## 帮助与版本

```sh
bit --help      # 完整帮助
bit -V          # bit 0.1.0
bit branch -h   # 单个命令那一节（含「会调用哪条 git 命令」）
```

两条命令都是交互式的，不接受任何参数：`bit branch foo`、`bit commit -m x` 一律报错并给出直路（`要直接建分支请用 git switch -c <名字>` / `要直接提交请用 git commit`）。

## 退出码

| 码 | 情形 | 流向 |
| --- | --- | --- |
| 0 | 成功；`-h` / `--help` / `-V` / `--version` | stdout（命令成功时输出的是 git 的） |
| 2 | 用法层失败：无参数、未知命令、未知选项、多余参数 | stderr |
| 1 | 运行期失败：Esc 取消、非 TTY、无暂存内容、编辑器非 0 退出 | stderr |
| 128 等 | git 自身的失败原样透传（重名分支 = 128） | git 的 stderr |
| 130 | Ctrl-C（信号语义），bit 不加文案 | — |

记忆点：**2 = 你还没让 bit 开始做事；1 = bit 开始做事后失败或被取消；128+ = git 说的。**

bit 自己说的话全是中文（`错误：` / `已取消：`、全角标点），git 的原文一字不改——语言本身就是来源标记。

## 边界与出路

- **不做透传**：`bit add .` 报错、不转发；其它 git 命令请直接用 git。
- **不做脚本化路径**：交互需要 TTY（非 TTY 直接拒绝、退出码 1）；脚本里请用 git。
- **v0.1 只服务本机**：提交风格档位（plain / subject+body 等）与发布分发不在范围内。

## 文档

- [`CONTEXT.md`](./CONTEXT.md) —— 术语表：分支类型、静默规范化、命令面、退出码分层……的规范叫法
- [`docs/adr/`](./docs/adr) —— 三个关键决策：bit 自拉编辑器并自校验、bit 承担分支名规范校验、命令面自成一格
- [`docs/research/`](./docs/research) —— 研究笔记：Conventional Branch、Conventional Commits、pty e2e 选型
- [`AGENTS.md`](./AGENTS.md) —— 面向 agent 的仓库约定

## 开发

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test        # 单测 + pty e2e（驱动真实二进制跑交互路径）
```

CI（[`.github/workflows/ci.yml`](./.github/workflows/ci.yml)）在 ubuntu / macos / windows 三平台跑同样三道门。
