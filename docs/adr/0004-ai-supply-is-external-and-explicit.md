# AI 供给外置且显式授权：bit 的联网从不是默认项

bit 从 v0.1 的离线工具变为 v0.2 的可联网工具，但联网能力不预置任何端点：AI 供给（提供商、端点、模型、密钥）一律由用户跑 `bit login` 写进本机配置后才存在；没有配置就是「未配置」，v0.1 两条路径零回归。此后 AI 能力只在两个显式点发起请求——`bit branch` 的中文描述翻译（配置可用即自动触发）与 `bit commit --gen`（旗标本身就是「可以把暂存 diff 发给所配端点」的授权）。不做遥测，不内置默认端点，不因缺配置拦截原有流程。

## Considered Options

- **内置默认端点 / 免费额度**（否）：等于 bit 代用户选定数据接收方；「无配置零回归」与「不内置任何默认端点」因此是硬约束。
- **OS keyring 存密钥**（否，v0.2）：明文 TOML 换零平台依赖与三平台 CI 的确定性；keyring 不在 v0.2 范围内。
- **引入 AI SDK（genai / async-openai / openai_dive 等）**（否）：只需要 OpenAI 兼容的 `POST /chat/completions` 与 `GET /models` 两个窄调用，薄客户端（ureq + serde_json，同步、无 tokio）比全异步 SDK 栈小得多，也与 bit 逐问逐答的交互同构。
- **AI 失败静默降级回手写 / 跳过确认**（否）：失败即运行期错误（退出码 1）并指路 `bit login`；不自动重试、不静默降级（见 `CONTEXT.md` 的「翻译失败」「生成失败」）。

## Consequences

- 数据出界只在用户看得见的两个点：描述翻译发「规范化 + 前缀剥离后的描述段」（附所选类型作上下文），`--gen` 发过滤、截断后的暂存 diff + stat + 当前分支名；v0.2 不脱敏，边界写进 README「AI 配置」。
- 配置读取是惰性的：不用到 AI 能力就不受配置影响——坏文件也拦不住不带 `--gen` 的 `bit commit` 与纯 ASCII 的 `bit branch`；用到 AI 能力时才区分「未配置」（指路 `bit login`）与「配置错误」（点名要点与路径），两者都是运行期 1。
- AI 只产出字段（type / scope / breaking / subject / body），组装、复核与提交仍归 bit：`--gen` 在编辑器定案后追加一次合规复核，记在 [ADR-0001](./0001-bit-owns-the-editor-and-validation.md) 的 v0.2 修订；命令面从两个命令扩成三个，记在 [ADR-0003](./0003-bit-owns-its-command-surface.md) 的 v0.2 修订。
- 供给调用参数随本决策冻结：非流式、连接 10 秒 / 读取 90 秒、不自动重试；`response_format: json_object` 按静态能力表，不确定时提示词兜底。

决定过程见 [决议 · AI 供给栈与 bit login 形态](https://github.com/p2mm2p/bit/issues/19)、[行为 · 配置存储与优先序](https://github.com/p2mm2p/bit/issues/25)、[行为 · 描述翻译细则](https://github.com/p2mm2p/bit/issues/23)、[行为 · 提交消息生成细则（--gen）](https://github.com/p2mm2p/bit/issues/24)。
