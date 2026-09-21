//! 抛弃型原型：`bit login` 向导的交互与文案（ticket #26 的产物，不是实现）。
//!
//! 它只回答一个问题——「向导的手感与文案该长什么样」：
//!
//! - **不联网**：`GET /models` 与连通性验证全是打桩，`--fail=` 可以演示 401 / 网络错误 / 模型 404；
//! - **不写文件**：结尾只按提交时的形态打印摘要（含配置路径），不碰真配置；
//! - **不校真 key**：`Password` 是 inquire 本体，粘贴、掩码、Esc 都走真行为。
//!
//! 跑法（原型分支 `prototype/26-login-wizard`）：
//!
//! ```text
//! cargo run --example login_stub                       # 主路径：DeepSeek，/models 成功
//! cargo run --example login_stub -- --provider=zhipu   # /models 失败 → 回退手输
//! cargo run --example login_stub -- --provider=custom  # 自定义 base_url
//! cargo run --example login_stub -- --provider=ollama  # 本地免密钥
//! cargo run --example login_stub -- --reconfig         # 重配：当前值作默认
//! cargo run --example login_stub -- --fail=auth        # 401：回 key 输入
//! cargo run --example login_stub -- --fail=network     # 网络错误：报错退出 1
//! cargo run --example login_stub -- --fail=model       # 模型 404：回模型输入
//! cargo run --example login_stub -- --variant=plain    # 提供商菜单去掉一行说明
//! cargo run --example login_stub -- --variant=quiet    # 回退手输时不解释原因
//! ```
//!
//! 屏幕网格抓取：`cargo test --test prototype_capture`（产物在 `docs/prototype/26-grids/`）。

use std::fmt;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use inquire::validator::Validation;
use inquire::{InquireError, Password, PasswordDisplayMode, Select, Text};

/// 运行期失败的退出码（沿 ADR-0003）。
const EXIT_RUNTIME: u8 = 1;

/// 打桩的网络时延：让「正在…」这类进度行在屏幕上真的看得见。
const MODELS_DELAY: Duration = Duration::from_millis(600);
const VERIFY_DELAY: Duration = Duration::from_millis(900);

/// 重配演示用的当前配置（写死；真实实现从配置文件读）。
const CURRENT: Stored = Stored {
    provider: "deepseek",
    base_url: "https://api.deepseek.com",
    model: "deepseek-v4-pro",
    api_key: "sk-proj-abc12347890xyz",
};

struct Stored {
    provider: &'static str,
    #[allow(dead_code)]
    base_url: &'static str,
    model: &'static str,
    api_key: &'static str,
}

// ---------------------------------------------------------------------------
// 打桩的提供商目录（事实取自 docs/research/ai-providers.md；模型清单是假数据）
// ---------------------------------------------------------------------------

/// `/models` 在向导里的两种下场。
#[derive(Clone, Copy)]
enum Models {
    /// 拉得到：给菜单选。
    Menu(&'static [&'static str]),
    /// 拉不到：回退手输，给建议模型（空串＝没有建议）。
    Manual(&'static str),
}

#[derive(Clone, Copy)]
struct Provider {
    /// 落盘 `provider = "…"` 的值（#25 的九种之一）。
    key: &'static str,
    /// 菜单显示名。
    name: &'static str,
    /// 菜单里的一句说明。
    hint: &'static str,
    /// 预置端点；`custom` 为空，要手输。
    base_url: &'static str,
    models: Models,
}

/// 24 个 OpenRouter 假模型：只为把「分页 + 输入筛选」演出来，不是真实清单。
const OPENROUTER_FAKE: &[&str] = &[
    "anthropic/claude-4.6-sonnet",
    "anthropic/claude-4.6-haiku",
    "deepseek/deepseek-v4",
    "deepseek/deepseek-v4-pro",
    "google/gemini-3.5-pro",
    "google/gemini-3.5-flash",
    "meta-llama/llama-4.2-405b",
    "meta-llama/llama-4.2-70b",
    "mistralai/mistral-large-3",
    "moonshotai/kimi-k3",
    "openai/gpt-5.2",
    "openai/gpt-5.2-mini",
    "openai/o5-pro",
    "prism-ml/ternary-bonsai-2-27b",
    "qwen/qwen3.8-max",
    "qwen/qwen3.8-plus",
    "x-ai/grok-5",
    "x-ai/grok-5-mini",
    "z-ai/glm-5.3",
    "z-ai/glm-5.3-flashx",
    "z-ai/glm-5.3-flash",
    "deepseek/deepseek-v4-flash",
    "moonshotai/kimi-k3-turbo",
    "mistralai/mistral-small-4",
];

/// 9 家：8 个预置 + 自定义，顺序沿用「决议 · AI 供给栈与 bit login 形态」。
const PROVIDERS: &[Provider] = &[
    Provider {
        key: "deepseek",
        name: "DeepSeek",
        hint: "国内直连",
        base_url: "https://api.deepseek.com",
        models: Models::Menu(&["deepseek-chat", "deepseek-reasoner", "deepseek-v4-pro"]),
    },
    Provider {
        key: "kimi",
        name: "Kimi（Moonshot）",
        hint: "国内直连",
        base_url: "https://api.moonshot.cn/v1",
        models: Models::Menu(&["kimi-k3", "kimi-k3-turbo"]),
    },
    Provider {
        key: "zhipu",
        name: "智谱 GLM",
        hint: "国内直连",
        base_url: "https://open.bigmodel.cn/api/paas/v4/",
        models: Models::Manual("GLM-5.3"),
    },
    Provider {
        key: "dashscope",
        name: "通义千问",
        hint: "国内直连",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        models: Models::Manual("qwen3.8-max"),
    },
    Provider {
        key: "openai",
        name: "OpenAI",
        hint: "官方 API",
        base_url: "https://api.openai.com/v1",
        models: Models::Menu(&["gpt-5.2", "gpt-5.2-mini", "o5-pro"]),
    },
    Provider {
        key: "openrouter",
        name: "OpenRouter",
        hint: "多模型聚合",
        base_url: "https://openrouter.ai/api/v1",
        models: Models::Menu(OPENROUTER_FAKE),
    },
    Provider {
        key: "siliconflow",
        name: "SiliconFlow",
        hint: "国内直连",
        base_url: "https://api.siliconflow.cn/v1",
        models: Models::Manual("deepseek-ai/DeepSeek-V4-Flash"),
    },
    Provider {
        key: "ollama",
        name: "Ollama（本地）",
        hint: "本机运行，免密钥",
        base_url: "http://localhost:11434/v1",
        models: Models::Menu(&["qwen3:8b", "llama3.2:3b"]),
    },
    Provider {
        key: "custom",
        name: "自定义 OpenAI 兼容",
        hint: "手输 base_url",
        base_url: "",
        models: Models::Manual(""),
    },
];

// ---------------------------------------------------------------------------
// 演示开关
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fail {
    None,
    /// 连通性验证 401：回 key 输入。
    Auth,
    /// 连不上：报错退出 1。
    Network,
    /// 模型 404：回模型输入。
    Model,
}

struct Options {
    /// `- -reconfig`：演示重配，当前值作默认。
    reconfig: bool,
    /// `--variant=plain`：提供商菜单不带说明。
    plain: bool,
    /// `--variant=quiet`：回退手输时不解释原因。
    quiet: bool,
    fail: Fail,
    /// `--provider=<key>`：菜单的起始光标落在哪家（省去演示时按方向键）。
    start: Option<String>,
}

impl Options {
    fn parse() -> Options {
        let mut options = Options {
            reconfig: false,
            plain: false,
            quiet: false,
            fail: Fail::None,
            start: None,
        };
        for arg in std::env::args().skip(1) {
            match arg.as_str() {
                "--reconfig" => options.reconfig = true,
                "--variant=plain" => options.plain = true,
                "--variant=quiet" => options.quiet = true,
                "--fail=auth" => options.fail = Fail::Auth,
                "--fail=network" => options.fail = Fail::Network,
                "--fail=model" => options.fail = Fail::Model,
                other => {
                    if let Some(key) = other.strip_prefix("--provider=") {
                        options.start = Some(key.to_string());
                    } else {
                        eprintln!("原型参数只认文档里那几条；未识别：{other}");
                        std::process::exit(2);
                    }
                }
            }
        }
        options
    }
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let options = Options::parse();
    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Halt::Cancelled) => {
            eprintln!("已取消：未做任何改动。");
            ExitCode::from(EXIT_RUNTIME)
        }
        // Ctrl-C 是信号语义，bit 不加文案（沿 v0.1）。
        Err(Halt::Interrupted) => ExitCode::from(130),
        // 具体文案已经打印过了。
        Err(Halt::Failed) => ExitCode::from(EXIT_RUNTIME),
    }
}

enum Halt {
    /// Esc：一句中文、退出码 1。
    Cancelled,
    /// Ctrl-C：退出码 130、无文案。
    Interrupted,
    /// 已经打印过具体错误。
    Failed,
}

fn run(options: &Options) -> Result<(), Halt> {
    // 重配时先给一行上下文，再进菜单（变体：不给这一行）。
    if options.reconfig {
        eprintln!(
            "当前配置：{} / {}",
            display_name(CURRENT.provider),
            CURRENT.model
        );
    }

    let provider = prompt_provider(options)?;
    let base_url = if provider.base_url.is_empty() {
        prompt_base_url()?
    } else {
        provider.base_url.to_string()
    };

    // 重配且没换家时，key 有「留空保持」的默认。
    let current_key = (options.reconfig && provider.key == CURRENT.provider)
        .then_some(CURRENT.api_key);

    let mut auth_retried = false;
    let mut model_retried = false;

    'auth: loop {
        let _key = prompt_key(provider, current_key)?;
        loop {
            let model = prompt_model(provider, options)?;

            eprintln!("正在验证连通性…");
            sleep(VERIFY_DELAY);
            match options.fail {
                Fail::Auth if !auth_retried => {
                    auth_retried = true;
                    eprintln!("认证失败（401）：API Key 无效或已过期，请重新输入。");
                    continue 'auth;
                }
                Fail::Network => {
                    eprintln!("错误：连不上 {base_url}（无法建立连接）。");
                    eprintln!("检查网络或代理设置，稍后重跑 bit login。");
                    return Err(Halt::Failed);
                }
                Fail::Model if !model_retried => {
                    model_retried = true;
                    eprintln!("模型不可用（404）：{model}。请换一个模型。");
                    continue;
                }
                _ => {
                    print_summary(provider, &base_url, &model);
                    return Ok(());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 各步骤
// ---------------------------------------------------------------------------

/// 带标签的行：inquire 用 `Display` 渲染、也用同一段文本做模糊筛选。
struct Row<T> {
    item: T,
    label: String,
}

impl<T> fmt::Display for Row<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn prompt_provider(options: &Options) -> Result<&'static Provider, Halt> {
    let rows: Vec<Row<&'static Provider>> = PROVIDERS
        .iter()
        .map(|provider| Row {
            item: provider,
            label: if options.plain {
                provider.name.to_string()
            } else {
                format!("{}｜{}", provider.name, provider.hint)
            },
        })
        .collect();
    let cursor = options
        .start
        .as_deref()
        .and_then(|key| PROVIDERS.iter().position(|provider| provider.key == key))
        .unwrap_or(0);

    let selected = skippable(
        Select::new("提供商", rows)
            .with_page_size(PROVIDERS.len())
            .with_starting_cursor(cursor)
            .with_help_message("输入可筛选，回车确认")
            .prompt_skippable(),
    )?;
    Ok(selected.item)
}

fn prompt_base_url() -> Result<String, Halt> {
    skippable(
        Text::new("base_url")
            .with_placeholder("https://…/v1")
            .with_help_message("OpenAI 兼容端点，含 /v1 之类的版本段")
            .with_validator(|input: &str| {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    return Ok(Validation::Invalid("base_url 不能为空".into()));
                }
                match trimmed.split_once("://") {
                    Some(("http" | "https", host)) if !host.is_empty() => Ok(Validation::Valid),
                    _ => Ok(Validation::Invalid(
                        "需以 http:// 或 https:// 开头，并带主机名".into(),
                    )),
                }
            })
            .prompt_skippable(),
    )
}

fn prompt_key(provider: &Provider, current_key: Option<&str>) -> Result<String, Halt> {
    let local = provider.key == "ollama";
    let help = if local {
        Some("本地 Ollama 无需密钥，直接回车（写入占位值 ollama）".to_string())
    } else {
        current_key.map(|key| format!("已配置（{}），留空保持不变", mask(key)))
    };

    let mut prompt = Password::new("API Key")
        .with_display_mode(PasswordDisplayMode::Masked)
        .without_confirmation();
    if let Some(help) = &help {
        prompt = prompt.with_help_message(help);
    }
    if !local && current_key.is_none() {
        prompt = prompt.with_validator(|input: &str| {
            if input.trim().is_empty() {
                Ok(Validation::Invalid("API Key 不能为空".into()))
            } else {
                Ok(Validation::Valid)
            }
        });
    }

    let answer = skippable(prompt.prompt_skippable())?;
    if answer.trim().is_empty() {
        if local {
            return Ok("ollama".to_string());
        }
        if let Some(key) = current_key {
            return Ok(key.to_string());
        }
    }
    Ok(answer)
}

fn prompt_model(provider: &Provider, options: &Options) -> Result<String, Halt> {
    eprintln!("正在获取模型列表…");
    sleep(MODELS_DELAY);

    match provider.models {
        Models::Menu(models) => {
            // 重配且没换家：光标落在当前模型上。
            let cursor = if options.reconfig && provider.key == CURRENT.provider {
                models
                    .iter()
                    .position(|model| *model == CURRENT.model)
                    .unwrap_or(0)
            } else {
                0
            };
            let mut rows: Vec<Row<Option<&'static str>>> = models
                .iter()
                .map(|model| Row {
                    item: Some(*model),
                    label: (*model).to_string(),
                })
                .collect();
            rows.push(Row {
                item: None,
                label: "手动输入模型名…".to_string(),
            });

            let selected = skippable(
                Select::new("模型", rows)
                    .with_page_size(10)
                    .with_starting_cursor(cursor)
                    .with_help_message("输入可筛选，回车确认")
                    .prompt_skippable(),
            )?;
            match selected.item {
                Some(model) => Ok(model.to_string()),
                None => prompt_model_manual(provider, options),
            }
        }
        Models::Manual(_) => prompt_model_manual(provider, options),
    }
}

fn prompt_model_manual(provider: &Provider, options: &Options) -> Result<String, Halt> {
    let default = match provider.models {
        Models::Manual(default) => default,
        Models::Menu(_) => "",
    };
    let placeholder = if default.is_empty() {
        "模型名".to_string()
    } else {
        default.to_string()
    };
    let help = if options.quiet {
        "请手动输入模型名".to_string()
    } else if default.is_empty() {
        "未能获取模型列表，请手动输入模型名（咨询你的提供商）".to_string()
    } else {
        "未能获取模型列表，请手动输入（占位符为建议值）".to_string()
    };

    skippable(
        Text::new("模型")
            .with_placeholder(&placeholder)
            .with_help_message(&help)
            .with_validator(|input: &str| {
                if input.trim().is_empty() {
                    Ok(Validation::Invalid("模型名不能为空".into()))
                } else {
                    Ok(Validation::Valid)
                }
            })
            .prompt_skippable(),
    )
}

fn print_summary(provider: &Provider, base_url: &str, model: &str) {
    println!("已保存 AI 供给：{} / {model}", provider.name);
    println!("端点：{base_url}");
    println!("配置文件：{}", config_path());
    eprintln!("[原型] 未联网、未写文件；上面三行按真实形态打印。");
}

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------

/// Esc → `Ok(None)`；Ctrl-C → 130；其余交互异常 → 运行期失败。
fn skippable<T>(result: Result<Option<T>, InquireError>) -> Result<T, Halt> {
    match result {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(Halt::Cancelled),
        Err(InquireError::OperationInterrupted) => Err(Halt::Interrupted),
        Err(other) => {
            eprintln!("错误：交互界面出错（{other}）。");
            Err(Halt::Failed)
        }
    }
}

/// `sk-abcdef…wxyz` 这种回显：首 3 + 尾 4。
fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "••••".to_string();
    }
    let head: String = chars[..3].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

fn display_name(key: &str) -> &'static str {
    PROVIDERS
        .iter()
        .find(|provider| provider.key == key)
        .map(|provider| provider.name)
        .unwrap_or("自定义 OpenAI 兼容")
}

/// 只用于展示的配置路径（真实解析在实现 ticket）。
fn config_path() -> String {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return format!("{}\\bit\\config.toml", appdata.to_string_lossy());
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return format!("{}/bit/config.toml", xdg.to_string_lossy());
    }
    if let Some(home) = std::env::var_os("HOME") {
        return format!("{home}/.config/bit/config.toml", home = home.to_string_lossy());
    }
    "（无法解析配置路径）".to_string()
}
