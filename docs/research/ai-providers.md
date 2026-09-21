Part of #17 · ticket #22

# 研究 · OpenAI 兼容供给面事实（端点 / 模型 / 能力矩阵）

> 问题：8 家预置提供商的 OpenAI 兼容面究竟长什么样？哪些差异会影响 bit v0.2 的薄客户端（ureq + serde_json）与 `bit login` 向导？
>
> 本文只引一手来源（官方文档 / 官方 OpenAPI spec / 官方源码与 README），每条论断后附出处。抓取与探活在本机（Windows / PowerShell）完成；除 OpenRouter 的公开 `GET /models` 外没有用任何 API key 做真实调用。部分官方文档站是 JS 渲染的 SPA，本机抓不到正文——这些条目标为**未核实**，不用推断补全。

---

## 0. 结论速览（TL;DR）

1. **8 家预置都讲 OpenAI 兼容 `chat/completions`，认证都是 `Authorization: Bearer`**；但能力面不一致，出入集中在两处：`GET /models` 与 `response_format`。
2. **`GET /models` 覆盖不全**：OpenAI / DeepSeek / Moonshot / OpenRouter / Ollama 有且已核实；**智谱与通义千问的官方 OpenAI 兼容文档未列该接口**；SiliconFlow 未核实。→ `bit login` 的「拉模型菜单」必须有静默回退手输，且回退不是罕见路径。
3. **`response_format` 不能假设家家都有 `json_object`**：OpenRouter 文档只承诺 `json_schema`（未列 `json_object`）；通义与 Moonshot 的 `json_object` 还要求提示词里出现「JSON」字样才不报错。→ 提示词 + 解析的兜底是**必需**而非可选。
4. **Ollama 本地是天然的免 key 通道**：`http://localhost:11434/v1`，`/v1/models` 与 `response_format` 都在兼容清单里，认证可传任意值（被忽略）——开发与验收不用花一分钱。
5. **`ureq` 默认 Agent 自动读代理环境变量**（`ALL_PROXY` → `HTTPS_PROXY` → `HTTP_PROXY`，含小写；`NO_PROXY` 自动生效），bit 不需要自己实现代理。
6. **Anthropic 与 Gemini 都提供官方 OpenAI 兼容端点**，但本机无法读取/连通其文档正文，内容未核实；v0.2 不预置，只作「自定义」示例。

---

## 1. 能力矩阵

| 提供商 | OpenAI 兼容 base_url | `GET /models` | `response_format` | 认证 | 文档中出现的模型示例 |
| --- | --- | --- | --- | --- | --- |
| OpenAI | `https://api.openai.com/v1` | ✅ `operationId: listModels` | ✅ `json_object`（JSON mode）+ `json_schema`（Structured Outputs） | `Bearer $OPENAI_API_KEY` | 由 `/models` 选择 |
| DeepSeek | `https://api.deepseek.com` | ✅ `GET /models` | ✅ `{"type":"json_object"}` | `Bearer` | `deepseek-v4-pro`（pricing 页） |
| Kimi / Moonshot | `https://api.moonshot.cn/v1` | ✅ `GET /v1/models` | ✅ `text` / `json_object` / `json_schema`（用 `json_object` 时须在提示词提「JSON」，原文截断待核） | `Bearer <token>` | `kimi-k3` |
| 智谱 GLM | `https://open.bigmodel.cn/api/paas/v4/` | ⚠️ 兼容文档未列（按不支持处理） | 兼容文档未逐项列；平台声明兼容 OpenAI 生态 | `Bearer`（API Key） | `GLM-5.3` / `GLM-5.3-Flash` / `GLM-5.3-FlashX` / `GLM-5.2` |
| 通义千问 / DashScope | `https://dashscope.aliyuncs.com/compatible-mode/v1`（另有工作空间专属新域名） | ⚠️ 兼容文档未列（按不支持处理） | ✅ `{"type":"json_object"}`（提示词必须含「JSON」字样）+ JSON Schema 模式 | `Bearer` | `qwen3.8-max`、`qwen-plus` |
| OpenRouter | `https://openrouter.ai/api/v1` | ✅ 公开、免认证（本机实测 200，446 个模型） | ⚠️ 文档只列 `json_schema`（未列 `json_object`） | `Bearer` | 由 `/models` 选择 |
| SiliconFlow | `https://api.siliconflow.cn/v1` | ⚠️ 未核实（文档站未找到条目） | ✅ `json_object` + `json_schema` | `Bearer {API Key}` | `deepseek-ai/DeepSeek-V4-Flash` |
| Ollama（本地） | `http://localhost:11434/v1` | ✅ `/v1/models`、`/v1/models/{model}` | ✅ 在 `/v1/chat/completions` 支持清单内；原生 `format` 另有 JSON / JSON Schema | 本地免认证（`api_key` 必填但被忽略）；云端用 `OLLAMA_API_KEY` | 本机已拉取的模型 |
| Anthropic（扩展） | 官方「OpenAI SDK compatibility」层，文档入口已核实、正文未核实 | 未核实 | 未核实 | 未核实 | 未核实 |
| Gemini（扩展） | `https://generativelanguage.googleapis.com/v1beta/openai/`（官方页存在，本机不可达） | 未核实 | 未核实 | 未核实 | 未核实 |

---

## 2. 逐家证据

### 2.1 OpenAI

- `servers: https://api.openai.com/v1`；`/models` 存在、`operationId: listModels`。出处：官方 OpenAPI spec <https://github.com/openai/openai-openapi>（本次读取 `master` 的 `openapi.yaml`）。
- `response_format` 两种：`{"type":"json_object"}` 是「older JSON mode」，`{"type":"json_schema", ...}` 是 Structured Outputs；spec 原文：*"Setting to `{ "type": "json_object" }` enables JSON mode, which ensures the message the model generates is valid JSON."*
- 认证示例：`-H "Authorization: Bearer $OPENAI_API_KEY"`（spec 内多处）。

### 2.2 DeepSeek

- base_url：文档首页参数表 `base_url (OpenAI) = https://api.deepseek.com`（同表另列 `base_url (Anthropic)`）。出处：<https://api-docs.deepseek.com/> 。
- `GET /models`：List Models 文档页（`GET /models`）。出处：<https://api-docs.deepseek.com/api/list-models> 。
- JSON mode：JSON Output 文档页，明确 `response_format` 设为 `{'type': 'json_object'}`。出处：<https://api-docs.deepseek.com/guides/json_mode> 。
- 错误码页面（表体未逐字核）：400 Invalid Format、401 Authentication Fails、402 Insufficient Balance、422 Invalid Parameters、429 Rate Limit、500 Server Error、503 Server Overloaded。出处：<https://api-docs.deepseek.com/quick_start/error_codes> 。
- 在售模型示例 `deepseek-v4-pro`。出处：<https://api-docs.deepseek.com/quick_start/pricing> 。

### 2.3 Kimi / Moonshot

- base_url 与认证：Chat API 页 cURL 示例 `https://api.moonshot.cn/v1/chat/completions` + `Authorization: Bearer <token>`。出处：<https://platform.moonshot.cn/docs/api/chat> 。
- `GET /v1/models`：List Models 页（cURL `https://api.moonshot.cn/v1/models`）。出处：<https://platform.moonshot.cn/docs/api/list-models> 。
- JSON 模式：Chat 页「JSON Mode」段，三种取值 `text` / `json_object` / `json_schema`；使用 `json_object` 时须在 system/user 提示词里带 JSON 相关说明（页面摘录截断，细节待核）。
- 模型示例 `kimi-k3`（Chat 页请求体）。

### 2.4 智谱 GLM

- base_url 与用法：`base_url="https://open.bigmodel.cn/api/paas/v4/"`。出处：<https://docs.bigmodel.cn/cn/guide/develop/openai/introduction.md> 。
- 该兼容文档通篇未出现 `GET /models`（全文检索无命中）→ 按「不支持，回退手输」处理。
- 模型总览：`GLM-5.3`（旗舰）、`GLM-5.3-Flash`、`GLM-5.3-FlashX`、`GLM-5.2` 等。出处：<https://docs.bigmodel.cn/cn/guide/start/model-overview.md> 。
- 错误码（业务码 / HTTP）：1000→401 身份验证失败、1001→401 缺 Authentication、1113→429 欠费、1210→400 参数错、1211→400 模型不存在。出处：<https://docs.bigmodel.cn/cn/api/api-code.md> 。

### 2.5 通义千问 / DashScope

- base_url：北京 `https://dashscope.aliyuncs.com/compatible-mode/v1`；文档同时提示迁移到工作空间专属域名（如 `https://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/compatible-mode/v1`）。出处：<https://help.aliyun.com/zh/model-studio/developer-reference/compatibility-of-openai-with-dashscope> 。
- 兼容文档未列 `GET /models`（检索无命中）；模型清单由单独的「支持的模型列表」页维护。
- JSON 模式：`response_format` 设 `{"type":"json_object"}`；且 System/User Message 必须含「JSON」关键词（不区分大小写），否则报错 `'messages' must contain the word 'json'`；另有 JSON Schema 模式。出处：<https://help.aliyun.com/zh/model-studio/json-mode> 。
- 模型示例 `qwen3.8-max`（兼容页请求体；页内注释「此处以 qwen-plus 为例」）。

### 2.6 OpenRouter

- base_url `https://openrouter.ai/api/v1`；`GET /models` 公开（本机实测 200、446 个模型、样例 id `prism-ml/ternary-bonsai-2-27b`、`z-ai/glm-5.3-flashx` 等）。出处：<https://openrouter.ai/api/v1/models> 。
- 结构化输出：`response_format` + `{"type":"json_schema", "json_schema": {...}}`，按模型支持；**文档未列 `json_object`**。出处：<https://openrouter.ai/docs/features/structured-outputs> 。
- 认证：Bearer（与 OpenAI 一致）。

### 2.7 SiliconFlow

- base_url 与认证：cURL `https://api.siliconflow.cn/v1/chat/completions` + `Authorization: Bearer {账户 API Key}`。出处：<https://docs.siliconflow.cn/cn/api-reference/chat-completions/chat-completions> 。
- `response_format`：同页参数表列 Text / JSON schema / JSON object 三种，正文写明 `{"type":"json_object"}` 启用 JSON 模式、支持 `json_schema` 的模型建议优先用 schema。
- 模型示例 `deepseek-ai/DeepSeek-V4-Flash`（同页）。
- `GET /models`：文档站未找到对应条目（本机未核实）→ 按回退手输处理。

### 2.8 Ollama

- 本地兼容端 `http://localhost:11434/v1`，`api_key='ollama'` 必填但被忽略；`/v1/models`、`/v1/models/{model}`、`/v1/chat/completions` 均存在；`response_format` 在 `/v1/chat/completions` 的支持清单里。出处：<https://github.com/ollama/ollama/blob/main/docs/api/openai-compatibility.mdx> 。
- 认证：本地 `http://localhost:11434` 不需要认证；云端 `https://ollama.com/v1` 用 `OLLAMA_API_KEY`（Bearer）。出处：<https://github.com/ollama/ollama/blob/main/docs/api/authentication.mdx> 。
- 原生结构化输出用 `format`（可给 `"json"` 或 JSON Schema）。出处：<https://github.com/ollama/ollama/blob/main/docs/capabilities/structured-outputs.mdx> 。
- 错误面示例：404 = model doesn't exist。出处：<https://github.com/ollama/ollama/blob/main/docs/api/errors.mdx> 。

### 2.9 Anthropic / Gemini（扩展，未核实）

- Anthropic：官方文档索引 `docs.claude.com/llms.txt` 里列有「OpenAI SDK compatibility」页（<https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk.md>）。本机抓到的是未渲染的 SPA（461KB HTML、无正文），**端点与用法未核实**。
- Gemini：官方页 <https://ai.google.dev/gemini-api/docs/openai> 存在（文档入口已见），但本机到 `ai.google.dev` / `generativelanguage.googleapis.com` 的连接均超时/失败（≥126s），**未核实**。
- 结论：v0.2 不预置这两家；「自定义 OpenAI 兼容」的 base_url 手输路径天然覆盖它们（用户自己填）。

---

## 3. 代理与网络（ureq 3）

- `ureq` 默认 `Agent` **自动读取代理环境变量**：`ALL_PROXY` → `HTTPS_PROXY` → `HTTP_PROXY`（含小写变体）；`NO_PROXY` 自动生效（支持精确主机、通配符等）。出处：ureq 官方 README「Proxying」段 <https://github.com/algesten/ureq>；`Proxy::try_from_env` 文档 <https://docs.rs/ureq/latest/ureq/struct.Proxy.html> 。
- 超时：ureq 提供 connect / read 超时配置（`Config`），具体默认值与 bit 的取值留给实现 ticket（E1）实测。

---

## 4. 对 bit v0.2 的直接含义（只列约束，不做决策）

1. **`bit login` 的模型菜单要接受「拉不到列表」是常态**：OpenAI / DeepSeek / Moonshot / OpenRouter / Ollama 可拉；智谱 / 通义 / SiliconFlow 预计回退手输。静默回退（「决议 · AI 供给栈与 bit login 形态」）因此是主路径之一，不是边角。
2. **JSON 契约必须带非 JSON 模式兜底**：不能假设 `response_format: json_object` 家家可用（OpenRouter、智谱未确认）；通义 / Moonshot 还要求提示词包含 JSON 说明。提示词本身就要把「只输出 JSON」写死。
3. **错误映射以 HTTP 状态码为主键**：401（认证）、400/422（参数/模型名，智谱用业务码 1210/1211）、429（限流，智谱 1113 欠费）、5xx（服务端）、网络/超时；错误体形状各家不同且未逐家核实，正文只做提示附加。
4. **默认模型占位符**只在文档见过示例的几家给（`kimi-k3`、`qwen3.8-max`、`GLM-5.3`、`deepseek-v4-pro`、`deepseek-ai/DeepSeek-V4-Flash`）；OpenRouter / Ollama 直接走 `/models`；其余留空提示手输。
5. **代理零成本**：ureq 默认行为已覆盖 `ALL_PROXY` / `HTTPS_PROXY` / `HTTP_PROXY` / `NO_PROXY`，E1 不需要自实现。
6. **Ollama 本地可作为开发/验收通道**：无 key、有 `/models`、有 `response_format`，适合 E8 真机清单里「真 provider」之前的一档。

---

## 5. 未核实清单（留给实现/实测）

- SiliconFlow `GET /models` 是否存在（路径未找到）。
- 智谱 `GET /models` 是否真的不存在（兼容文档未列 ≠ 一定没有）。
- Moonshot `json_object` 对提示词的强制要求细节（页面摘录截断）。
- Anthropic / Gemini 兼容端点的 base_url、`/models`、`response_format`、认证。
- ureq `Config` 的默认超时值与自定义方式。
