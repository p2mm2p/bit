Part of #2 · ticket #3

# 研究 · Conventional Commits 的类型体系与格式细则

> 问题：`bit commit` 应内置哪一档提交类型清单？交互生成的消息要满足哪些格式规则？
>
> 本文只引一手来源：Conventional Commits v1.0.0 规范原文、Angular / AngularJS 官方提交约定、
> commitlint 官方 config-conventional、git 官方手册。每个论断后附出处。
>
> **文件位置说明**：本仓库尚无 research 笔记约定（现有目录只有 `docs/agents/`，领域文档规划为
> `docs/adr/`，见 `AGENTS.md` 与 `docs/agents/domain.md`）。因此新建 `docs/research/` 作为研究笔记目录，
> 文件名 `conventional-commits.md`，与 ticket 主题对应。

---

## 0. 结论速览（TL;DR）

1. **类型清单**：内置 spec 推荐的 11 类 —— `feat / fix / docs / style / refactor / perf / test / build / ci / chore / revert`。
   这正是 commitlint `config-conventional` 的 `type-enum`，也是规范 item 4 所指向的类型表。
2. **`hotfix` 不属于提交类型**。它既不在规范里，也不在 commitlint / Angular 的任一类型表中；它是 git-flow 的
   **分支名前缀**（`hotfix-*`）。热修复本身应提交为 `fix`。
3. **交互合规**：「选类型 → 选/输 scope → 用 git 编辑器预填 `type(scope): ` → 用户补 subject/body → 交给 git 提交」
   与规范的强制骨架一致，方向正确；但**预填会让消息“非空”**，从而绕过 git 自带的“空消息中止”保护，
   `bit` 必须在编辑器返回后自行校验 subject 非空、`type: `（无 scope）不得写成 `type(): ` 等。

---

## 1. 提交类型体系

### 1.1 规范原文的规定

Conventional Commits 1.0.0 对类型的规定只有两条硬性 + 一条开放性：

- 提交**必须**以类型开头（`feat`、`fix` 等名词），后接可选 scope、可选 `!`、以及**必需的**终止冒号加空格。
- `feat` **必须**用于新增功能；`fix` **必须**用于修 bug。
- 除 `feat` / `fix` 外的类型**允许**使用（`docs` 等）；规范不强制规定其它类型，且它们（除含 breaking change 外）
  在语义化版本中**没有隐含效果**。

出处：规范正文 Specification 第 1、2、3、14 条，以及 Summary 部分
<https://www.conventionalcommits.org/en/v1.0.0/#specification> 。

规范 item 4 明确把“其它类型”的出处指向 commitlint 的 `config-conventional`，并说明它基于 Angular 约定
（规范链接到 Angular 仓库的某个历史 revision 的 CONTRIBUTING）：

> *types* other than `fix:` and `feat:` are allowed, for example
> [@commitlint/config-conventional](https://github.com/conventional-changelog/commitlint/tree/master/@commitlint/config-conventional)
> (based on the [Angular convention](https://github.com/angular/angular/blob/22b96b9/CONTRIBUTING.md#-commit-message-guidelines))
> recommends `build:`, `chore:`, `ci:`, `docs:`, `style:`, `refactor:`, `perf:`, `test:`, and others.

出处：规范 Summary 第 4 条 <https://www.conventionalcommits.org/en/v1.0.0/#summary> 。

### 1.2 类型表的实际出处链

规范 → `@commitlint/config-conventional` → Angular 约定。这条链上有三个可引的一手来源：

- **commitlint `config-conventional` 的 `type-enum`（规范点名的类型表）**，完整列表为：
  `build, chore, ci, docs, feat, fix, perf, refactor, revert, style, test`。
  出处：<https://github.com/conventional-changelog/commitlint/blob/master/%40commitlint/config-conventional/README.md#type-enum> 。
- **Angular 当前提交约定**（`contributing-docs/commit-message-guidelines.md`）的类型表为：
  `build | ci | docs | feat | fix | perf | refactor | test`（不含 `chore` / `style` / `revert`；
  当前 Angular 把 `style` 从表中去掉，`revert` 单独按 `revert: <被回滚提交的 header>` 处理）。
  出处：<https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md#type> 。
- **AngularJS 提交约定**（`DEVELOPERS.md`）的旧类型表为：
  `feat, fix, docs, style, refactor, perf, test, chore` —— 这是 `chore` / `style` 语义的一手定义处。
  出处：<https://github.com/angular/angular.js/blob/master/DEVELOPERS.md#type> 。
- 规范 Summary 第 4 条当初链接的 Angular 历史 revision（`22b96b9`）类型表为
  `build, ci, docs, feat, fix, perf, refactor, style, test`。
  出处：<https://github.com/angular/angular/blob/22b96b9/CONTRIBUTING.md#type> 。

> 注意：`config-conventional` 的 `type-enum` 是**最贴近规范**的一份清单（规范原文直接点名它，且它同时覆盖了
> `chore`、`style`、`revert` 三个 Angular 现表已删/单列的类型）。因此本笔记以它作为推荐清单的基准。

### 1.3 推荐给 `bit commit` 内置的类型清单（11 类）

按 commitlint `config-conventional` 的集合，逐条一句话语义（语义定义取自 Angular / AngularJS 一手来源）：

| 类型 | 一句话语义 | 语义出处 |
| --- | --- | --- |
| `feat` | 新增功能（对应 SemVer MINOR） | 规范 §2 / Angular |
| `fix` | 修 bug（对应 SemVer PATCH） | 规范 §3 / Angular |
| `docs` | 仅文档改动 | Angular |
| `style` | 不影响代码含义的改动（空白、格式化、缺分号等） | AngularJS |
| `refactor` | 既不修 bug 也不加功能的代码改动 | Angular |
| `perf` | 提升性能的代码改动 | Angular |
| `test` | 新增或修正测试 | Angular |
| `build` | 影响构建系统或外部依赖的改动 | Angular |
| `ci` | 改动 CI 配置文件与脚本 | Angular |
| `chore` | 构建流程或辅助工具 / 库的杂项改动（如文档生成） | AngularJS |
| `revert` | 回滚此前的提交；正文写 `This reverts commit <SHA>.` | 规范 FAQ / AngularJS |

出处汇总：

- 语义（feat/fix/docs/refactor/perf/test/build/ci）：<https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md#type>
- 语义（style/chore/revert）：<https://github.com/angular/angular.js/blob/master/DEVELOPERS.md#type>
- `revert` 类型建议 + `This reverts commit <hash>.` 写法：规范 FAQ「How does Conventional Commits handle revert commits?」
  <https://www.conventionalcommits.org/en/v1.0.0/#how-does-conventional-commits-handle-revert-commits>
  以及 AngularJS DEVELOPERS.md 的 Revert 小节。

> 关于 `chore` 与 `style`：规范本身**没有**给语义定义，只把它们列进推荐名单；语义定义来自 AngularJS 约定，
> commitlint 只是把它们保留在 `type-enum` 里。这是“权威来源中最接近的定义”，不是规范强制。

### 1.4 「`hotfix` 是否属于提交类型」——明确结论

**结论：`hotfix` 不是 Conventional Commits 的提交类型，`bit commit` 不应把它放进类型清单。**

依据：

1. **规范里没有 `hotfix`**。规范只强制 `feat` / `fix`，其余为推荐集合，且该集合里不含 `hotfix`。
   规范 item 4 列出的是 `build, chore, ci, docs, style, refactor, perf, test`。
   出处：<https://www.conventionalcommits.org/en/v1.0.0/#summary> 。
2. **commitlint `config-conventional` 的 `type-enum` 没有 `hotfix`**（11 项见 §1.2）。
   出处：<https://github.com/conventional-changelog/commitlint/blob/master/%40commitlint/config-conventional/README.md#type-enum> 。
3. **Angular / AngularJS 的类型表都没有 `hotfix`**。
   出处：Angular <https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md#type> ；
   AngularJS <https://github.com/angular/angular.js/blob/master/DEVELOPERS.md#type> 。
4. **`hotfix` 的真实身份是 git-flow 的分支名前缀**：git-flow 的支撑分支有 feature / release / hotfix 三类，
   hotfix 分支的命名约定是 `hotfix-*`，用于从 `master` 上拉出、紧急修复线上版本，而不是一种提交类型。
   出处：Vincent Driessen, *A successful Git branching model*, "Hotfix branches"，
   <https://nvie.com/posts/a-successful-git-branching-model/#hotfix-branches> 。

**落地建议**：线上紧急问题照常提交为 `fix`（语义化版本 PATCH）；“紧急”这一层含义由**分支命名**（`hotfix-*`，
见 ticket #1）承载，而不是由提交类型承载。规范也建议一个提交只表达一个类型，必要时拆成多个提交
（FAQ「What do I do if the commit conforms to more than one of the commit types?」）。

### 1.5 类型清单是“项目约定”，不是规范强制

规范的 FAQ 明确说：规范允许团队自定义并随时间调整类型
（<https://www.conventionalcommits.org/en/v1.0.0/#might-conventional-commits-lead-developers-to-limit-the-type-of-commits-they-make-because-theyll-be-thinking-in-the-types-provided>）。
所以“内置哪一档”本质是 `bit` 的项目约定。建议采用上表 11 类，理由是它与主流工具链
（commitlint、changelog 生成器、语义化版本推算）**零冲突**；若要裁剪，宁可减成员也不要发明新类型
（例如不要把 `hotfix`、`wip`、`release` 当类型）。

> 交互排序属 UX 取舍，非规范问题：规范与 commitlint 都是字母序；AngularJS 按
> `feat, fix, docs, style, refactor, perf, test, chore`。交互列表若想降低认知负担，可按常用度排序
> （`feat, fix` 置顶），但最终清单成员应与上表一致。

---

## 2. 交互生成消息必须满足的格式细则

先给出规范强制的骨架，再逐段给规则，并区分「规范 MUST」与「Angular / commitlint 风格规则」。

规范骨架（规范 Summary）：

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

出处：<https://www.conventionalcommits.org/en/v1.0.0/#summary> 。

### 2.1 Header 骨架

- **规范 MUST**：提交必须以类型开头，后接**可选 scope、可选 `!`、以及必需的终止冒号加空格**；
  description **必须**紧随类型/scope 前缀之后。类型/scope 之后是 `<description>`。
  出处：规范 §1、§5 <https://www.conventionalcommits.org/en/v1.0.0/#specification> 。
- **Angular 风格**：header 为 `<type>(<scope>): <short summary>`，`<type>` 与 `<short summary>` 必填，
  `(<scope>)` 可选。出处：<https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md#commit-message-header> 。
- **scope 为空时必须整个省略括号**：写成 `type: subject`，**不能**写成 `type(): subject`
  —— 因为 scope（若给出）必须是描述代码区段的名词（规范 §4），空括号不构成合法 scope。
- **大小写**：规范围绕 type 的大小写只说“任意大小写皆可，但最好保持一致”；**实现方不得把组成提交的各信息单元
  视为大小写敏感**（`BREAKING CHANGE` 除外，见 §2.6）。
  出处：规范 §15 <https://www.conventionalcommits.org/en/v1.0.0/#specification> ，
  与 FAQ「Are the types in the commit title uppercase or lowercase?」。
  commitlint 的**风格档**则强制 type 小写（`type-case: lowerCase`，见 §2.7）。
- **行长**：commitlint `config-conventional` 强制 header ≤ 100 字符（`header-max-length`）；git 官方建议首行
  “尽量不超过 50 字符”。出处：<https://github.com/conventional-changelog/commitlint/blob/master/%40commitlint/config-conventional/README.md#header-max-length> ，
  git-commit DISCUSSION <https://git-scm.com/docs/git-commit#_discussion> 。

### 2.2 scope

- **规范 MUST/MAY**：scope **可以**在类型之后给出；若给出，**必须**是描述代码区段的名词、包裹在圆括号里，
  例如 `fix(parser):`。规范**没有**规定 scope 的字符集或枚举。出处：规范 §4。
- **Angular 风格**：scope 应是受影响的 npm 包名（对 changelog 读者而言）；Angular 给它一份固定清单
  （小写、连字符，如 `compiler`、`platform-browser`、`service-worker`），并对跨包改动允许**空 scope**
  （例如跨所有包的 `test` / `refactor`，以及不属于特定包的 `docs`）。
  出处：<https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md#scope> 。
- **AngularJS 旧约定**：scope “可以是任何指明本次提交改动位置的东西”，并允许用 `*` 表示影响多个 scope。
  出处：<https://github.com/angular/angular.js/blob/master/DEVELOPERS.md#scope> 。
- **实现建议**：规范未限字符集，故 `bit` 只需保证 scope 不含会破坏 header 语法的字符：`(`、`)`、`:`，
  以及首尾空白 / 换行。至于大小写，规范明确不敏感，但 Angular 惯例是小写。commitlint `config-conventional`
  **未**对 scope 设任何规则（无 `scope-enum` / `scope-case`），所以 scope 的形态由 `bit` 自己约定。

### 2.3 subject（description）

- **规范 MUST**：description **必须**紧跟在类型/scope 前缀的冒号加空格之后，是对改动的简短概括。
  出处：规范 §5。
- **Angular 风格**（规范未强制，来自 Angular/commitlint）：
  - 用祈使句、现在时：`change`，而非 `changed` / `changes`；
  - 首字母**不**大写；
  - 句尾**不加**句点 `.`。
  出处：<https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md#summary> 。
  - commitlint 把这三点落实为 `subject-case`（禁 sentence/start/pascal/upper-case）、
    `subject-full-stop`（禁尾点）；另强制 `subject-empty`（subject 不得为空）。
    出处：<https://github.com/conventional-changelog/commitlint/blob/master/%40commitlint/config-conventional/README.md> 。
- **git 语义**：commit 消息第一行（到首个空行前）是标题，被 git 全流程当作 title 使用（如 `format-patch` 的
  Subject）。出处：git-commit DISCUSSION。

### 2.4 body

- **规范 MAY / MUST（若存在）**：更长的正文**可以**提供；若有，**必须**与 description 之间隔**一个空行**。
  正文是自由格式，可包含任意数量的、以换行分隔的段落。出处：规范 §6、§7。
- **Angular 风格**：正文同样用祈使句现在时；Angular **当前**约定正文除 `docs` 外为必填且不少于 20 字符。
  出处：<https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md#commit-message-body> 。
- **commitlint 风格**：`body-leading-blank`（正文前应有空行，warning 级）、`body-max-line-length` 100
  （含 URL 的行豁免）。
- **注意**：规范**不要求**写 body；Angular 的“body 必填”是 Angular 自己的约定，`bit` 不应强制。

### 2.5 footer

- **规范 MUST/MAY**：footer **可以**有一个或多个，位于**正文之后空一行**；每个 footer 由「一个词元 token +
  `:<空格>` 或 `<空格>#` 分隔符 + 字符串值」组成（灵感来自 git trailer）。token 中的空白**必须**用 `-` 代替
  （如 `Acked-by`），唯一例外是 `BREAKING CHANGE` 也可作为 token。footer 的值可含空格与换行，解析在遇到下一个
  合法 token/分隔符对时终止。出处：规范 §8、§9、§10。
- **Angular 风格**：footer 用于 breaking change、deprecation，以及引用关闭/相关的 issue 与 PR
  （`Fixes #<n>` / `Closes #<n>`）。`DEPRECATED: ` 亦以 footer 形式书写。
  出处：<https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md#commit-message-footer> 。
- **commitlint 风格**：`footer-leading-blank`（footer 前应有空行，warning）、`footer-max-line-length` 100。
- **git trailer 依据**：git 的 trailer 需位于消息末尾、前面至少一个空行，key 与分隔符间允许空格，
  值可折行续写。出处：<https://git-scm.com/docs/git-interpret-trailers#_description> 。
- **规范给的 `Refs: #123` 写法**：footer 引用 issue 时形如 `Refs: #123`、`Fixes #123`，
  规范示例即 `<footer>` 块里的 `Reviewed-by: Z` / `Refs: #123`。
  出处：规范 Examples 与 §8。

### 2.6 `!` 与 `BREAKING CHANGE:`

- **规范 MUST**：breaking change **必须**在「类型/scope 前缀」**或**「footer 条目」二者之一中指出。
  出处：规范 §11。
- **作为 footer 时**：**必须**是大写 `BREAKING CHANGE` + 冒号 + 空格 + 描述，例如
  `BREAKING CHANGE: environment variables now take precedence over config files`。
  `BREAKING CHANGE` **必须**大写（§15 的唯一大小写例外）；`BREAKING-CHANGE` 作为 footer token 与之同义（§16）。
  出处：规范 §12、§15、§16；Angular 同样要求 footer 以 `BREAKING CHANGE: ` 开头。
- **作为前缀时**：**必须**是在 `:` **之前緊接**一个 `!`；若用了 `!`，footer 的 `BREAKING CHANGE:`
  **可以**省略，此时提交的 description 即用于描述该破坏性改动。
  出处：规范 §13。
- **规范示例**：`feat!: send an email...`、`feat(api)!: send an email...`、`feat!: drop support for Node 6` +
  footer `BREAKING CHANGE: use JavaScript features not available in Node 6.`
  出处：<https://www.conventionalcommits.org/en/v1.0.0/#examples> 。
- **语义化版本**：任何类型只要含 breaking change，无论类型为何，都对应 MAJOR。出处：规范 FAQ「How does this relate to SemVer?」。

### 2.7 两种“档位”：规范 MUST vs 工具风格

规范自身的 MUST 较少；Angular/commitlint 叠加了一批**风格规则**（不违背规范、但比规范更严）。
`bit` 需明确自己实现到哪一档：

| 规则 | 规范强制？ | Angular / commitlint |
| --- | --- | --- |
| `type[(scope)][!]: description` 骨架、冒号+空格 | MUST（§1、§5） | — |
| description 紧随前缀、非空 | MUST（§5） | commitlint `subject-empty` |
| body 前空一行 | MUST（§6） | commitlint `body-leading-blank`（warning） |
| footer 前空一行 | MUST（§8） | commitlint `footer-leading-blank`（warning） |
| scope 为名词、圆括号包裹 | MUST（§4） | 无字符/枚举约束 |
| `!` 紧接 `:` 之前 | MUST（§13） | — |
| `BREAKING CHANGE:` 大写 + 冒号空格 | MUST（§12、§15） | — |
| type 小写 | 否（§15 明说不敏感） | `type-case: lowerCase` |
| subject 祈使、首字母不大写、无尾点 | 否 | `subject-case` / `subject-full-stop` |
| header ≤ 100；body/footer 行 ≤ 100 | 否 | `header/body/footer-max-line-length` |

出处：规范 §1–§16 <https://www.conventionalcommits.org/en/v1.0.0/#specification> ；
commitlint 规则 <https://github.com/conventional-changelog/commitlint/blob/master/%40commitlint/config-conventional/README.md> 。

---

## 3. 「预填 `type(scope): ` 再让用户补完」这一步是否合规？有哪些坑？

**总体判断：合规，方向正确。** 预填的正是规范强制的头部前缀
（`type[(scope)][!]: ` + 一个空格，规范 §1），用户补 description 与 body/footer 后即得合法结构。
把编辑器交给 git 本体（`git commit -e -m "<前缀>"`，或经 `-t <模板>` / `COMMIT_EDITMSG`）也与本仓库
“包装 git、保留其确切语义”的取向一致：编辑器仍按
`GIT_EDITOR` → `core.editor` → `VISUAL` → `EDITOR` 的顺序解析。出处：git-commit 的
*Environment and Configuration Variables* <https://git-scm.com/docs/git-commit> 。

但有以下**具体的坑**，多数源于「预填让消息变成非空」：

1. **空 subject 会绕过 git 的安全网（最重要）。**
   git 默认在消息为空时中止提交；但预填的 `type(scope): ` 经 cleanup 后并非空串，于是提交照常进行，
   产生 `type(scope):` 这种**缺 description** 的非法 header——违反规范 §5，也会被 commitlint 的
   `subject-empty` 判失败。默认 cleanup 模式 `strip` 会“去掉行尾空白”，所以用户若什么都不打，
   行尾那个空格被抹掉，恰好剩下 `type(scope):`。
   出处：git-commit 的 `--cleanup=strip` <https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---cleanupmode> ；
   规范 §5；commitlint `subject-empty`。
   → **`bit` 必须在编辑器返回后校验 description 非空**；若用户原样退出（消息等于预填），应中止或让用户重试。

2. **scope 为空时不能预填成 `type(): `。**
   规范要求 scope（如给出）是名词（§4），空括号不是合法 scope。scope 留空时预填应为 `type: `。

3. **`!` 的位置。** 若要标注 breaking change，`!` 必须紧接 `:` 之前（§13）。预填 `type(scope): ` 后，
   用户需手动把 `!` 插到括号与冒号之间，容易插错或漏插。
   → 建议在交互里提供显式的“breaking change?”开关，直接预填 `type(scope)!: `。

4. **`#` 开头的内容会被当注释删掉。** git 的注释字符默认是 `#`（`core.commentChar`），默认 `strip` cleanup
   会删除注释行。因此 body/footer 里**以 `#` 开头**的行（例如正文第一行直接写 `#123 修复了…`）会被静默删除。
   引用 issue 要用 trailer 形式（`Refs: #123` / `Fixes #123`），不能把 `#` 放在行首——规范自身的示例正是
   `Refs: #123`。
   出处：git-commit `--cleanup` / `commit.cleanup`；规范 Examples。

5. **`scissors` 模式会截断。** 若用户/仓库把 `commit.cleanup` 设为 `scissors`，`# ------------------------ >8 ------------------------`
   之后的内容会被整体丢弃。默认不是 scissors，但 `bit` 不应依赖注释模板做提示。
   出处：git-commit `--cleanup=scissors`。

6. **`commit.template` 可能污染预填。** 若仓库或用户配置了 `commit.template`，git 会把该模板内容也放进
   编辑器缓冲区，可能与你预填的前缀叠在一起。`bit` 应显式给消息（`-m`）或在编辑器返回后重新解析、校验 header。
   出处：git-commit `-t/--template` 与 `commit.template`。

7. **`-m` 预填 + `-e` 时“未编辑即提交”不再被 git 兜底。** git 的“若用户未编辑模板则中止”仅适用于
   以模板提供消息的情形；当消息由 `-m`/`-F` 给定时该行为不生效。因此无论走哪条路，**校验责任都在 `bit`**。
   出处：git-commit `-t/--template` 描述。

8. **编码。** 提交消息惯例用 UTF-8；仓库写作默认中文，subject/body 用中文没问题，但要确保写出的是 UTF-8
   （git 明确不支持 UTF-16/32、EBCDIC，以及 GBK/Shift-JIS/Big5 等 CJK 多字节编码）。出处：git-commit DISCUSSION。

9. **行长与风格校验。** 若 `bit` 想保证“提交能过 commitlint”，需在编辑器返回后按 §2.7 的风格档自检
   （header ≤ 100、type 小写、subject 首字母不大写且无尾点、body/footer 前空行）。若不打算自检，也应明确
   文档化“`bit` 只保证规范 MUST，不保证 Angular 风格”。

10. **`cleanup` 档位取舍。** 为保留用户原样输入可考虑 `--cleanup=verbatim`，但它会**保留**注释行，把
    以 `#` 开头的行并入消息，反而更糟。建议沿用默认 `strip`，并把校验做在 `bit` 一侧。

---

## 4. 对 `bit commit` 的具体建议（供 ticket #4 拍板）

- **类型清单**：内置 §1.3 的 11 类；不含 `hotfix`。
- **预填规则**：scope 有值时 `type(scope): `，无值时 `type: `；提供 breaking change 选项时预填
  `type(scope)!: `（或 `type!: `）。
- **提交后校验**（编辑器返回、交付 git 之前）：description 非空；header 不出现空括号 `()`；若声明 breaking
  change，`!` 或 `BREAKING CHANGE:` 至少有一处且写法正确（大写）。
- **不强制 body**：规范与 commitlint 都不要求 body；Angular 的“body 必填”不采纳。
- **风格档待定**：是否顺带强制 type 小写 / subject 无尾点 / header ≤ 100，属实现档位选择，见 §2.7。

---

## 5. 关键出处（一手）

1. Conventional Commits 1.0.0 规范原文 —— <https://www.conventionalcommits.org/en/v1.0.0/>
   （Specification: <https://www.conventionalcommits.org/en/v1.0.0/#specification>）
2. Angular 提交消息约定 —— <https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md>
3. 规范点名的 Angular 历史 revision（`22b96b9`）CONTRIBUTING —— <https://github.com/angular/angular/blob/22b96b9/CONTRIBUTING.md#type>
4. AngularJS 提交约定（`chore` / `style` / `revert` 语义）—— <https://github.com/angular/angular.js/blob/master/DEVELOPERS.md#commits>
5. commitlint `@commitlint/config-conventional`（`type-enum` 与风格规则）—— <https://github.com/conventional-changelog/commitlint/blob/master/%40commitlint/config-conventional/README.md>
6. Vincent Driessen, *A successful Git branching model*（hotfix 分支）—— <https://nvie.com/posts/a-successful-git-branching-model/#hotfix-branches>
7. `git-commit` 手册（cleanup / template / editor / 编码）—— <https://git-scm.com/docs/git-commit>
8. `git-interpret-trailers` 手册（footer / trailer 结构）—— <https://git-scm.com/docs/git-interpret-trailers>
