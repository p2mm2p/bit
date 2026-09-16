# 分支名的规范级校验在 bit，git 的宽松不是许可

`bit branch` 生成的 `<type>/<desc>` 要过 Conventional Branch 的文法，而 git 的 ref 规则比规范宽松得多（实测 git 2.55 接受 `feature/UPPER`、`feature/-lead`、甚至中文名）——只把名字转交给 git，产物照样建得出来，却过不了规范校验器。于是规范级校验由 bit 自己承担（7 类类型前缀 + 描述段文法），不合规在输入框原地报错重输；大小写、空格、下划线这类日常输入按规范随附技能的「静默纠正」算法吸收，并在创建前的确认步骤回显纠正前后。重名、非仓库、detached HEAD 等 git 自身的语义则完全不预检、照旧交给 git。

## Considered Options

- **只 trim 后交给 git，合规与否不管**（否）：git 会照常建出 `feature/UPPER` 这类名字，用户以为规范生效、实际没有；问题要等到事后校验器（commit-check 之类）才暴露。
- **不纠正、任何不合规都报错**（否）：规范随附技能明确要求对大小写与分隔符做静默纠正；输入「Add OAuth Login」是常态，逐字报错是把规范当成输入格式、而不是约定。
- **重名也由 bit 预检**（否）：`git show-ref --verify --quiet refs/heads/<name>` 可行，但重名不是规范问题、不在 bit 的校验域；git 的 `fatal: a branch named 'x' already exists`（exit 128）已是唯一真相，预检只会造出第二套判断。

## Consequences

- bit 比 git 更严：git 能建的名字（大写、前导连字符、非 ASCII）bit 会拒绝——有意为之，规范优先。
- 校验与规范化都是纯函数（规范化 / 前缀剥离 / 文法校验 / 组装），进单测；与菜单的类型清单同源，避免两张表漂移。
- 静默纠正必须可见：确认步骤在发生过纠正时回显「由 "<原文>" 规范化」，否则用户看不到自己的输入被改成了什么。
- 名字的最终真相仍在 git：绕过 bit、直接用 git 建的分支不受本约定约束，bit 只约束自己这条路径。
- 与 [ADR-0001](./0001-bit-owns-the-editor-and-validation.md) 是同一条原则的两侧：bit 包装 git，但「合规」由 bit 负责，git 只负责执行与它自身的语义错误。

决定过程与实测证据（git 2.55.0.windows.5）见 [行为 · bit branch 细则](https://github.com/p2mm2p/bit/issues/7)。
