Part of #2 · ticket #1

# 研究 · 分支类型体系与命名格式

> 目标：为 `bit branch` 确定「内置哪一档分支类型清单」与「采用什么命名格式」。
> 只引用一手来源：规范原文、官方文档、一手仓库/代码。每条论断附出处链接，见文末「出处」。

## 0. 结论摘要

1. **存在正式规范**：Conventional Branch（当前 **1.1.0**）。它直接仿照 Conventional Commits 而来，把 `<type>/<description>` 固化为带 ABNF 文法与机器可读校验正则的规范。
2. **规范内置的用途类型只有 5 个**：`feature/`(别名 `feat/`)、`bugfix/`(别名 `fix/`)、`hotfix/`、`release/`、`chore/`。规范 FAQ 明确解释：**分支类型故意不像提交类型那样细**（没有 `build`/`ci`/`docs`/`style`/`refactor`），因为分支是临时的。
3. **用户初稿中的 `docs` / `test` 不在规范文法内**——用它们会生成 `commit-check` 判为不合规的分支名（规范正则只接受固定类型集合）。规范的解法是：**允许团队自定义类型**，但要求「文档化」。
4. **推荐 `bit branch` v0.1 内置规范的 5 个用途前缀**（feature / fix / hotfix / release / chore），`feat` 作为 `feature` 的别名输入；`docs`/`test`/`refactor` 等作为需要时再开启的「扩展类型」，默认不内置（理由见 §2、§6）。
5. **前缀选 `feature/` 而非 `feat/`**：`feature` 是规范的正名（canonical），`feat` 只是别名，且规范自带的 agent 技能明确说「Prefer the full names over aliases」。`feat/` 仍应被接受为输入别名。
6. **命名格式**：`<type>/<description>`，全小写，仅 `a-z 0-9 - .`，词间用单个连字符（kebab-case），禁止连续/首尾连字符与点，禁止下划线/空格/特殊字符；描述 2–5 个词、整名约 ≤50 字符。可有条件内嵌工单号 `feature/issue-123-new-login`。

---

## 1. 规范背景（一手）

### 1.1 Conventional Branch

- 定位：`"A specification for Git branch names that are human-readable, machine-parseable, and automation-friendly."` 结构为 `<type>/<description>`。出处：[spec README](https://github.com/conventional-branch/conventional-branch#readme)。
- 版本：官网首页当前为 **1.1.0**；`v1.0.0` 为存档版（[1.0.0 存档](https://conventionalbranch.org/v1.0.0/)）。1.1.0 相对 1.0.0 **无非破坏性变更**，主要新增「AI Agent Source Prefixes」（`ai/`、`copilot/`、`cursor/`、`claude/`、`codex/`）。出处：[1.1.0 首页 FAQ](https://conventionalbranch.org/)。
- **机器可读规范是本研究的权威校验依据**：`spec.json` 给出类型表、分隔符、ABNF 文法，以及**唯一锚定校验正则**（原文：`grammar.regex is the authoritative validator`）。出处：[spec.json](https://conventionalbranch.org/spec.json)（版本冻结地址 `https://conventionalbranch.org/v1.1.0/spec.json`）。
- 规范自身的用途前缀表与文法（原文摘录）：

  > `feature/` (or `feat/`): New features …… `bugfix/` (or `fix/`): Bug fixes …… `hotfix/`: Urgent fixes …… `release/`: For branches preparing a release …… `chore/`: For non-code tasks like dependency, docs updates

  出处：[1.1.0 首页](https://conventionalbranch.org/)。

- 规范明确「分支类型刻意比提交类型少」：

  > **Why aren't branch types as detailed as Conventional Commits (e.g., `build`, `ci`, `docs`, `style`, `refactor`)?** — Branches are different from commits—they are temporary and mainly used until merged. Introducing too many types for branches would be unnecessary and would make them harder to manage and remember.

  出处：[1.1.0 首页 FAQ](https://conventionalbranch.org/)。

- 规范允许自定义类型（但要求文档化）：

  > Yes. The specification defines a recommended set of types, but teams can define additional custom types to fit their workflow. It is important, however, to document custom types clearly so that all team members and automated tooling are aware of them.

  出处：[1.1.0 首页 FAQ](https://conventionalbranch.org/)。

- 名称转写与长度建议（规范随附的一手 agent 技能）：

  > Use **kebab-case** with 2-5 words …… Be descriptive but concise (~50 chars total) …… Bad: `fix-bug`, `new-feature` …… `[fix it silently]` Lowercase everything / Replace underscores and spaces with hyphens / Collapse consecutive hyphens / Strip leading/trailing hyphens

  出处：[SKILL.md](https://github.com/conventional-branch/conventional-branch/blob/main/skills/conventional-branch/SKILL.md)。

### 1.2 Conventional Commits（提交侧对照）

- 提交结构 `<type>[optional scope]: <description>`；`fix` 对应 SemVer `PATCH`，`feat` 对应 `MINOR`，`BREAKING CHANGE` 对应 `MAJOR`。出处：[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)。
- 规范只**强制** `feat` 与 `fix` 两个类型，其余类型「allowed」；它把扩展类型推荐权交给社区配置：

  > *types* other than `fix:` and `feat:` are allowed, for example @commitlint/config-conventional (based on the Angular convention) recommends `build:`, `chore:`, `ci:`, `docs:`, `style:`, `refactor:`, `perf:`, `test:`, and others.

  出处：[Conventional Commits 1.0.0 §Summary](https://www.conventionalcommits.org/en/v1.0.0/)。
- 上述社区配置 `@commitlint/config-conventional` 的 `type-enum` 权威取值（一手仓库）：

  > `['build','chore','ci','docs','feat','fix','perf','refactor','revert','style','test']`

  出处：[@commitlint/config-conventional README](https://github.com/conventional-changelog/commitlint/tree/master/@commitlint/config-conventional)。

### 1.3 git-flow（Vincent Driessen, 2010 + 2020 反思）

- 模型内置**两个长生命周期主分支** `master` 与 `develop`，以及三类支撑分支。原文：

  > The different types of branches we may use are: Feature branches / Release branches / Hotfix branches

  出处：[A successful Git branching model](https://nvie.com/posts/a-successful-git-branching-model/)。
- git-flow 的**分支命名约定**（注意其用连字符而非斜杠）：feature 分支「anything except `master`, `develop`, `release-*`, or `hotfix-*`」；release 分支 `release-*`；hotfix 分支 `hotfix-*`。出处同上。
- **作者 2020 年的反思**（一手更正）：对持续交付的软件应改用更简单的 GitHub flow，而非把 git-flow 硬套进去：

  > If your team is doing continuous delivery of software, I would suggest to adopt a much simpler workflow (like GitHub flow) instead of trying to shoehorn git-flow into your team.

  出处：[A successful Git branching model · Note of reflection](https://nvie.com/posts/a-successful-git-branching-model/)。

- 关键差异：git-flow 用 `release-1.2`/`hotfix-1.2.1`（连字符 + 版本号），**没有** `type/description` 的 `feature/` 前缀体系；`feature/`、`bugfix/`、`<type>/<desc>` 这套是 Conventional Branch 的贡献。

### 1.4 GitHub flow（主流轻量实践）

- GitHub 官方推荐的默认工作流；**不强制任何前缀**，只要求「短且描述性」的分支名。原文：

  > Create a branch …… A short, descriptive branch name enables your collaborators to see ongoing work at a glance. For example, `increase-test-timeout` or `add-code-of-conduct`.

  出处：[GitHub flow - GitHub Docs](https://docs.github.com/en/get-started/using-github/github-flow)。
- 这佐证：前缀体系是**可选约定**，不前缀也可行；`bit branch` 的价值在于把约定固化成默认。

---

## 2. 类型体系建议

**规范用途前缀全集（5 个，建议内置）：**

| 类型 | 别名 | 一句话语义 | 出处 |
|---|---|---|---|
| `feature/` | `feat/` | 新增功能或功能增强 | [spec.json](https://conventionalbranch.org/spec.json) "New features" |
| `fix/` | `bugfix/` | 修复缺陷（非紧急） | spec.json "Bug fixes"（规范正名为 `bugfix`，`fix` 为别名） |
| `hotfix/` | — | 生产环境紧急修复 | spec.json "Urgent production fixes" |
| `release/` | — | 准备一次发布（描述段可含版本号） | spec.json "Release preparation" |
| `chore/` | — | 非代码任务：依赖、文档、配置 | spec.json "Non-code tasks (dependencies, docs, config)" |

> 注：规范里 `bugfix`/`fix` 与 `feature`/`feat` 是「正名 + 别名」关系（`"type": "bugfix", "aliases": ["fix"]`）。因此**不要同时内置 `feature` 与 `feat`、或 `bugfix` 与 `fix` 两套并列项**，那样只是同义重复。

**扩展类型（规范允许、但不在 ABNF/正则内，建议「按需开启」而非默认内置）：**

| 类型 | 一句话语义 | 依据 |
|---|---|---|
| `docs/` | 仅文档改动 | commitlint 类型表 `docs` |
| `test/` | 仅测试改动 | commitlint 类型表 `test` |
| `refactor/` | 不改变外部行为的代码重构 | commitlint 类型表 `refactor` |
| `style/` | 不影响语义的格式/风格改动 | commitlint 类型表 `style` |
| `perf/` | 性能优化 | commitlint 类型表 `perf` |
| `ci/` | 持续集成/交付配置改动 | commitlint 类型表 `ci` |
| `build/` | 构建系统或依赖打包改动 | commitlint 类型表 `build` |

**为什么建议「内置 5 + 扩展按需」，而不是照抄提交类型全集（10–12 个）：**

1. 规范设计者已明确表态分支类型不宜照搬提交类型（§1.1 FAQ）。
2. 用提交类型全集会引入与规范**正则不兼容**的名字（`docs/`、`test/`…），使 `bit branch` 的产物无法通过规范自带的校验器 `commit-check`（[spec.json 正则](https://conventionalbranch.org/spec.json) 的 type 集合不含这些）。
3. 规范允许自定义类型，因此扩展类型**并非违规**，只是脱离了规范校验范围——这正是把它做成「可选档」而非默认项的合适理由。

---

## 3. 命名格式规范（供 `bit branch` 直接实现）

结构：`<type>/<description>`。分隔符 `/`（spec.json `"separator": "/"`）。

**硬性规则（照抄规范，非法即拒绝/静默纠正）：**

1. **全小写**：`a-z`、`0-9`、`-`、`.` 之外一律非法（[spec.json rules.case = "lowercase"](https://conventionalbranch.org/spec.json)）。
2. **词间连字符**：描述段用单个连字符连接单词（kebab-case）。
3. **禁止**连续连字符/点、首尾连字符/点（`feature/new--login`、`feature/-new-login`、`feature/new-login-` 均非法）。
4. **禁止**下划线、空格、其它特殊字符（`fix/header_bug`、`fix/header bug` 非法）。
5. **点号**仅用于版本号（规范惯例为 `release/v1.2.0`）；正则技术上允许任意描述的 `.`，但应仅限版本语义。
6. 描述段**不得为空**（文法 `desc-segment = 1*(ALPHA / DIGIT) ...`）。

**软性建议（规范随附技能）：**

- 描述 **2–5 个词**，整名约 **≤50 字符**；要「描述性但简洁」。
- 好例子：`add-oauth-login`、`fix-header-overflow`、`update-ci-config`；坏例子：`fix-bug`、`new-feature`。
- **工单号可选**，直接内嵌进描述段，如 `feature/issue-123-new-login`（规范 Basic Rule 4）。

**描述段转写算法（把用户自然语言→合法 description，规范技能要求「静默纠正」）：**

```
1. 转小写
2. 空格、下划线 → 连字符
3. 折叠连续连字符为单个（含剔除 - 与 . 的相邻/首尾情况）
4. 去首尾连字符与点
5. 校验：字符合法、非空、无连续/首尾分隔符（用 spec.json 的正则复核）
```

**推荐实现方式**：直接内联 [spec.json 的锚定正则](https://conventionalbranch.org/spec.json)（版本冻结地址 `…/v1.1.0/spec.json`）做最终校验，避免自造文法漂移：

```
^(?:main|master|develop|(?:feature|feat|bugfix|fix|hotfix|release|chore|ai|copilot|cursor|claude|codex)/[a-z0-9]+(?:\.[a-z0-9]+)*(?:-[a-z0-9]+(?:\.[a-z0-9]+)*)*)$
```

---

## 4. `feature/` vs `feat/`：结论

**结论：内置前缀用 `feature/`；把 `feat/` 作为可接受的输入别名。**

理由（全部来自规范本身）：

1. **正名 vs 别名**：规范里 `feature` 是 type，`feat` 是其 `aliases`（`"type": "feature", "aliases": ["feat"]`）。约定应默认正名。出处：[spec.json](https://conventionalbranch.org/spec.json)。
2. **技能层的显式指引**：规范随附的一手技能写 `Prefer the full names over aliases for consistency`。出处：[SKILL.md](https://github.com/conventional-branch/conventional-branch/blob/main/skills/conventional-branch/SKILL.md)。
3. **与提交侧的对应天然成立**：规范自己的对照表就写 `feature/add-login` ↔ `feat: add login page`——分支用 `feature/`、提交用 `feat:` 并不矛盾，反而正是规范给出的配法。出处：[SKILL.md · Relationship with Conventional Commits](https://github.com/conventional-branch/conventional-branch/blob/main/skills/conventional-branch/SKILL.md)。

> 反面权衡（供拍板）：若希望 `bit branch` 与 `bit commit` 的 **UI 文案完全同词**（都显示 `feat`），也可反过来默认 `feat/`。但这会让默认产物偏离规范正名，并和「Prefer the full names」相左。本研究的推荐仍是 `feature/`（+ 接受 `feat` 别名）。
>
> 同理，`fix` 是 `bugfix` 的别名；若采用 `feature/` 正名策略，`fix/` 作为 bugfix 的别名输入也应被接受。

---

## 5. 与 Conventional Commits 的对应关系

Conventional Branch 由 Conventional Commits 派生，二者「designed to be used together」（[spec README](https://github.com/conventional-branch/conventional-branch#readme)）。规范给出的映射（[SKILL.md](https://github.com/conventional-branch/conventional-branch/blob/main/skills/conventional-branch/SKILL.md)）与本研究扩展到扩展类型的建议：

| 分支类型 | 典型提交类型 | 说明 |
|---|---|---|
| `feature/` | `feat:` | 规范对照表直给 |
| `fix/`（`bugfix/`） | `fix:` | 规范对照表直给 |
| `chore/` | `chore:` | 依赖、文档、配置 |
| `release/` | `chore:`（如 `chore: release v1.2.0`） | 规范对照表直给 |
| `hotfix/` | `fix:`（紧急修复本质是 fix） | 语义推导 |
| `docs/` | `docs:` | 扩展类型 |
| `test/` | `test:` | 扩展类型 |
| `refactor/` | `refactor:` | 扩展类型 |
| `style/` | `style:` | 扩展类型 |
| `perf/` | `perf:` | 扩展类型 |
| `ci/` | `ci:` | 扩展类型 |
| `build/` | `build:` | 扩展类型 |

**要点**：分支类型是提交类型的**子集**（规范刻意如此，§1.1 FAQ）。因此 `bit` 的两个命令若共享一份类型数据，应是「分支 5 个 ⊂ 提交 10–12 个」，而非一一等同。

---

## 6. 对 `bit branch` 的具体建议（本 ticket 的交付结论）

1. **内置类型清单（默认菜单，5 个）**：`feature`、`fix`、`hotfix`、`release`、`chore`。
   - 与 Conventional Branch 1.1.0 的用途前缀全集一致，产物可通过规范校验器。
   - 接受别名输入：`feat`→feature、`bugfix`→fix。
   - 不内置「AI agent 前缀」（`ai`/`copilot`/…）——那是给 agent 自动建分支用的，非人机交互式 CLI 的核心场景；可作为后续可选。
2. **扩展类型档（按需开启，建议 v0.1 不默认）**：`docs`、`test`、`refactor`、`style`、`perf`、`ci`、`build`。启用时须在文档中声明为「自定义类型」（规范要求），并知晓它们超出规范正则。
3. **命名格式**：`<type>/<description>`；全小写；kebab-case；描述 2–5 词、约 ≤50 字符；工单号可选内嵌。
4. **转写**：按 §3 的五步算法做静默规范化，再用 spec.json 正则终校。
5. **`release/` 与 `hotfix/` 的取舍提醒**：这两个来自 git-flow 的重量级流程，其作者已建议持续交付场景改用 GitHub flow（§1.3）。若 `bit` 面向的是单人/持续交付仓库，可在 UI 里弱化它们；但作为规范类型仍应保留在清单中。

### 待用户拍板（写进地图的 "Not yet specified"）

- `docs` / `test` 是否进入 v0.1 内置（本研究建议：否，放入扩展档）——它与用户初稿（含 `docs`/`test`）存在冲突，需一次明确。
- 内置前缀用 `feature/` 还是 `feat/`（本研究建议 `feature/`）。
- 是否内置 `release/`（涉及 git-flow 式流程是否在范围内）。

---

## 出处（一手）

1. Conventional Branch 1.1.0 规范首页（含类型表、规则、ABNF、FAQ） — https://conventionalbranch.org/
2. Conventional Branch 机器可读规范 `spec.json`（types / separator / grammar.regex，权威校验器） — https://conventionalbranch.org/spec.json
3. Conventional Branch 规范仓库 README（定位、1.1.0 新特性、机器可读规范说明） — https://github.com/conventional-branch/conventional-branch
4. Conventional Branch 随附 agent 技能 `SKILL.md`（kebab-case、2–5 词、静默纠正、偏好正名、分支↔提交映射） — https://github.com/conventional-branch/conventional-branch/blob/main/skills/conventional-branch/SKILL.md
5. Conventional Commits 1.0.0 规范 — https://www.conventionalcommits.org/en/v1.0.0/
6. `@commitlint/config-conventional` README（`type-enum` 权威取值） — https://github.com/conventional-changelog/commitlint/tree/master/@commitlint/config-conventional
7. git-flow 原始文章 + 2020 Note of reflection（feature/release/hotfix、命名约定、持续交付反思） — https://nvie.com/posts/a-successful-git-branching-model/
8. GitHub flow（官方工作流文档，短描述性分支名，无强制前缀） — https://docs.github.com/en/get-started/using-github/github-flow

---

*文件位置说明：仓库无既有 research 笔记目录（现有 `docs/` 下仅 `docs/agents/`）。按本 ticket 建议落于 `docs/research/conventional-branch.md`，与 `docs/adr/`、`docs/agents/` 同属 `docs/` 体系。*
