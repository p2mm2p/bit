# bit 自行驱动编辑器，消息校验责任留在 bit

`bit commit` 预填规范前缀再开编辑器，而预填会让 git 的「空消息即中止」保护失效（实测：消息为 `feat(ui): ` 时 git 照常提交出 `feat(ui):`），校验因此必须在提交前发生；而 `git commit -e` 把「开编辑器 + 校验 + 提交」做成原子操作，bit 拿不到那个窗口。于是 bit 自己拉起编辑器、自己校验，再以 `git commit -F <消息文件> --cleanup=strip` 提交；编辑器是谁一律问 `git var GIT_EDITOR`，不自行复刻 git 的优先级。

## Considered Options

- **`git commit -e`，编辑器与提交全交给 git**（否）：bit 无法在提交前校验空 subject；提交后再 amend 不可接受。
- **bit 自建临时消息文件**（否）：位置与生命周期都要自己管，而 `.git/COMMIT_EDITMSG` 本就是 git 的消息草稿文件——用 `git rev-parse --path-format=absolute --git-path COMMIT_EDITMSG` 解析，linked worktree 下也正确。
- **`git commit -m <多行消息>`**（否）：Windows 上的命令行长度、引号与编码全要自己扛。

## Consequences

- 清洗也委派给 git：校验对象 = `git stripspace --strip-comments` 的输出（即 git 将保存的消息），bit 不自己写清洗逻辑；提交仍显式带 `--cleanup=strip`，保证 git 保存的与 bit 校验的是同一份语义。
- 清洗档位取 `strip`（对齐 git 直接 `git commit` 走编辑器的语义）：`#` 开头的内容行会被静默丢弃，因此种子里的注释提示行可以安全存在、也不会污染历史。
- bit 只做规范 MUST 级校验（description 非空）；不强制 type 小写、subject 尾点、header ≤ 100 等 Angular / commitlint 风格档。
- 光标落点由编辑器决定（vim 在第 1 行行首），用户自行到行尾补写；占位符方案因「忘了删就原样提交进历史」而被否。

决定过程与实测证据（git 2.55.0.windows.5）见 [行为 · bit commit 细则](https://github.com/p2mm2p/bit/issues/4)。

## 修订（v0.2）

`bit commit --gen` 在编辑器定案后追加一次与菜单等价的合规复核（type ∈ 11 类、scope 合法、subject 非空）；不过则走同一有界回环（带原文重开、未改动即放弃）。人工路径仍只查 description 非空。理由：AI 产出的只是字段，draft 属于不可信输入，组装与文法复核仍由 bit 负责（见 [ADR-0004](./0004-ai-supply-is-external-and-explicit.md)）。
