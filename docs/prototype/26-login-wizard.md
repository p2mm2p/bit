Part of #17 · ticket #26

# 原型 · bit login 向导交互与文案 —— 走查

> 抛弃型原型的说明页。产物：`examples/login_stub.rs`（可跑的 stub，全程打桩）+
> `tests/prototype_capture.rs`（ConPTY 抓屏夹具）+ 本文与 `26-grids/` 的终端网格截图。
> 全部只活在 `prototype/26-login-wizard` 分支，拍板后由实现 ticket 吸收、本分支丢弃。

## 怎么跑

```sh
cargo build --example login_stub
cargo run --example login_stub                       # 主路径：DeepSeek，/models 成功
cargo run --example login_stub -- --provider=zhipu   # /models 失败 → 回退手输
cargo run --example login_stub -- --provider=custom  # 自定义 base_url
cargo run --example login_stub -- --provider=ollama  # 本地免密钥
cargo run --example login_stub -- --reconfig         # 重配：当前值作默认
cargo run --example login_stub -- --fail=auth        # 401：回 key 输入
cargo run --example login_stub -- --fail=network     # 网络错误：报错退出 1
cargo run --example login_stub -- --fail=model       # 模型 404：回模型输入
cargo run --example login_stub -- --variant=plain    # 菜单只留名字
cargo run --example login_stub -- --variant=quiet    # 回退手输不解释原因
```

重新抓网格：`cargo test --test prototype_capture`（写回 `26-grids/`）。

## 流程总览

```
提供商 → [自定义] base_url → API Key → 试 /models
   成功 → 模型菜单（可筛选，末项「手动输入模型名…」）
   失败 → 手输模型（占位符给建议值，静默回退、不报错）
→ 连通性验证
   通过 → 成功摘要（含配置路径）
   401  → 回 API Key 输入
   404  → 回模型输入
   网络 / 429 / 5xx → 报错退出 1
Esc 任意一步取消（已取消：未做任何改动。 / 1）；Ctrl-C 由信号语义收场（130、无文案）。
```

## 网格截图

| 场景 | 截图 |
| --- | --- |
| 主路径：提供商菜单 | `26-grids/01-01-happy-provider-menu.txt` |
| 主路径：模型菜单 | `26-grids/03-01-happy-model-menu.txt` |
| 主路径：成功摘要 | `26-grids/04-01-happy-summary.txt` |
| 回退手输（智谱） | `26-grids/01-02-fallback-model-manual.txt` |
| 自定义 base_url | `26-grids/01-03-custom-base-url.txt` |
| 本地 Ollama：免密钥提示 | `26-grids/01-04-local-key-local.txt` |
| 本地 Ollama：模型菜单 | `26-grids/02-04-local-model-menu-local.txt` |
| OpenRouter：分页 + 筛选 | `26-grids/01-05-openrouter-model-menu-filtered.txt` |
| 401：回 key 输入 | `26-grids/01-06-auth-auth-401.txt` |
| 网络错误：退出 1 | `26-grids/01-07-network-network-error.txt` |
| 模型 404：回模型输入 | `26-grids/01-08-model-model-404.txt` |
| Esc 取消 | `26-grids/01-09-cancel-esc-cancel.txt` |
| 重配：菜单与顶部上下文 | `26-grids/01-10-reconfig-provider-menu-reconfig.txt` |
| 重配：key 留空保持 | `26-grids/02-10-reconfig-key-keep.txt` |
| 重配：模型光标落在当前值 | `26-grids/03-10-reconfig-model-menu-current.txt` |
| 变体：菜单只留名字 | `26-grids/01-11-plain-provider-menu-plain.txt` |
| 变体：回退不解释 | `26-grids/01-12-quiet-model-manual-quiet.txt` |

## 待拍板的点（推荐值加粗）

1. **提供商菜单一行说明**：**`名字｜短说明`**（推荐；9 项一屏放得下，不做分组）还是只有名字。
   说明文案：DeepSeek / Kimi / 智谱 / 通义 / SiliconFlow｜国内直连；OpenAI｜官方 API；
   OpenRouter｜多模型聚合；Ollama（本地）｜本机运行，免密钥；自定义｜手输 base_url。
2. **回退手输的说明**：**`未能获取模型列表，请手动输入（占位符为建议值）`** 还是安静版
   `请手动输入模型名`；没有建议值的自定义家是 `未能获取模型列表，请手动输入模型名（咨询你的提供商）`。
3. **key 输入**：**Masked（星号回显）+ 不二次确认 + 不提供 Ctrl+R 明文切换**；
   备选是 inquire 默认的 Hidden（完全无回显）。粘贴走 inquire 本体，无长度上限。
4. **重配**：**顶部一行 `当前配置：DeepSeek / deepseek-v4-pro`**；key 帮助行
   `已配置（sk-…0xyz），留空保持不变`（首 3 尾 4 掩码）；菜单与模型光标落在当前值。
5. **失败去向**：**401 回 key 输入、404 回模型输入、网络 / 429 / 5xx 报错退出 1**；
   备选一律退出 1 让用户重跑。
6. **成功摘要**：**三行**——`已保存 AI 供给：<显示名> / <模型>`、`端点：<base_url>`、
   `配置文件：<path>`。
7. **Ollama 空 key**：**写占位值 `ollama`**（本地上游要求 api_key 必填但忽略）。
8. **模型菜单末项**：**保留「手动输入模型名…」**，给 `/models` 拉回来但不合适的场景留 Escape。
9. **进度行**：**`正在获取模型列表…` / `正在验证连通性…`**；验证通过不再额外打「连通性正常」。

## 文案清单（草稿，供「命令面 · v0.2 增补」冻结）

| 情形 | 文案 | 去向 |
| --- | --- | --- |
| 提供商提示 | `提供商`（帮助：`输入可筛选，回车确认`） | 选择/Enter |
| 自定义地址 | `base_url`（占位 `https://…/v1`；帮助 `OpenAI 兼容端点，含 /v1 之类的版本段`；空→`base_url 不能为空`；无 scheme → `需以 http:// 或 https:// 开头，并带主机名`） | Esc/Enter |
| 密钥提示 | `API Key`；本地帮助 `本地 Ollama 无需密钥，直接回车（写入占位值 ollama）`；重配帮助 `已配置（sk-…0xyz），留空保持不变`；空→`API Key 不能为空` | Esc/Enter |
| 拉列表进度 | `正在获取模型列表…` | 进行时 |
| 模型菜单 | `模型`（帮助 `输入可筛选，回车确认`；末项 `手动输入模型名…`） | 选择/Enter |
| 回退手输 | `模型`（占位＝建议值；帮助见待拍板 2） | Esc/Enter |
| 验证进度 | `正在验证连通性…` | 进行时 |
| 401 | `认证失败（401）：API Key 无效或已过期，请重新输入。` | 回 key 输入 |
| 404 | `模型不可用（404）：<model>。请换一个模型。` | 回模型输入 |
| 网络 | `错误：连不上 <base_url>（无法建立连接）。` + `检查网络或代理设置，稍后重跑 bit login。` | 退出 1 |
| 429 / 5xx | 同形状，换状态：`错误：请求过于频繁（429），稍后重跑 bit login。` / `错误：服务端错误（5xx），稍后重跑 bit login。` | 退出 1 |
| 成功 | `已保存 AI 供给：<显示名> / <模型>` + `端点：<base_url>` + `配置文件：<path>` | 退出 0 |
| 取消 | `已取消：未做任何改动。` | 退出 1 |
| Ctrl-C | 无文案 | 退出 130 |

## 已知的毛边（不是决策，记录在案）

- 提供商菜单用 `｜` 连接，中文名宽度不一，右侧不严格对齐；v0.1 的 `{:<9}` 对齐在这行不通。
- 模型 404 后 stub 重新拉了一次 `/models`；真实实现可以复用已经拿到的列表，少一次调用。
- 验证请求的形状（最小 `chat/completions`？`max_tokens` 取多少）不归本 ticket，
  留给「实现 · AI 供给客户端」与「实现 · bit login 向导」。
