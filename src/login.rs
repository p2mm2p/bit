//! `bit login` 的领域数据与纯函数：9 家提供商目录、掩码、校验与逐字文案。
//!
//! 交互与文案冻结在 [原型 · bit login 向导交互与文案](https://github.com/p2mm2p/bit/issues/26)
//! 与 [命令面 · v0.2 增补](https://github.com/p2mm2p/bit/issues/27) 的登录节；
//! 预置端点与 `/models` 覆盖面的差异见 [docs/research/ai-providers.md](../../docs/research/ai-providers.md)。
//! 本模块只放纯函数与数据（可单测），向导编排与网络调用在 `flow`。

use std::path::Path;

/// 一家预置提供商（或「自定义」）；`key` 取 [`crate::config::PROVIDERS`] 的九种之一。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provider {
    /// 落盘 `provider = "…"` 的值。
    pub key: &'static str,
    /// 菜单显示名。
    pub name: &'static str,
    /// 菜单里的一句说明，与名字拼成 `名字｜短说明`。
    pub hint: &'static str,
    /// 预置端点；`custom` 为空串，向导会反问 base_url。
    pub base_url: &'static str,
    /// `/models` 拉不到时手输提示里的建议值；`None` 即占位符用「模型名」。
    pub suggestion: Option<&'static str>,
}

/// 「自定义」的显示名：菜单与兜底共用。
pub const CUSTOM_NAME: &str = "自定义 OpenAI 兼容";

/// 菜单顺序沿 [决议 · AI 供给栈与 bit login 形态](https://github.com/p2mm2p/bit/issues/19)（#26 拍板 1）。
pub const PROVIDERS: [Provider; 9] = [
    Provider {
        key: "deepseek",
        name: "DeepSeek",
        hint: "国内直连",
        base_url: "https://api.deepseek.com",
        suggestion: None,
    },
    Provider {
        key: "kimi",
        name: "Kimi（Moonshot）",
        hint: "国内直连",
        base_url: "https://api.moonshot.cn/v1",
        suggestion: None,
    },
    Provider {
        key: "zhipu",
        name: "智谱 GLM",
        hint: "国内直连",
        base_url: "https://open.bigmodel.cn/api/paas/v4/",
        suggestion: Some("GLM-5.3"),
    },
    Provider {
        key: "dashscope",
        name: "通义千问",
        hint: "国内直连",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        suggestion: Some("qwen3.8-max"),
    },
    Provider {
        key: "openai",
        name: "OpenAI",
        hint: "官方 API",
        base_url: "https://api.openai.com/v1",
        suggestion: None,
    },
    Provider {
        key: "openrouter",
        name: "OpenRouter",
        hint: "多模型聚合",
        base_url: "https://openrouter.ai/api/v1",
        suggestion: None,
    },
    Provider {
        key: "siliconflow",
        name: "SiliconFlow",
        hint: "国内直连",
        base_url: "https://api.siliconflow.cn/v1",
        suggestion: Some("deepseek-ai/DeepSeek-V4-Flash"),
    },
    Provider {
        key: "ollama",
        name: "Ollama（本地）",
        hint: "本机运行，免密钥",
        base_url: "http://localhost:11434/v1",
        suggestion: None,
    },
    Provider {
        key: "custom",
        name: CUSTOM_NAME,
        hint: "手输 base_url",
        base_url: "",
        suggestion: None,
    },
];

/// 按 key 找一家；`Ready` 配置里的 provider 必然命中（`config::load` 已校过）。
pub fn provider(key: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|provider| provider.key == key)
}

/// 摘要与顶部上下文里的显示名；防御性兜底到「自定义 OpenAI 兼容」。
pub fn display_name(key: &str) -> &'static str {
    match provider(key) {
        Some(provider) => provider.name,
        None => CUSTOM_NAME,
    }
}

/// 首 3 尾 4 掩码（#26 拍板 4）；短于等于 8 个字符两头兼顾不了，回四个圆点。
pub fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "••••".to_string();
    }
    let head: String = chars[..3].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// base_url 校验（#26 文案表）：空与不合形两句冻结文案。
pub fn validate_base_url(input: &str) -> Result<(), &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(BASE_URL_EMPTY);
    }
    match trimmed.split_once("://") {
        Some(("http" | "https", host)) if !host.is_empty() => Ok(()),
        _ => Err(BASE_URL_MALFORMED),
    }
}

/// 回退手输的占位符：有建议值给建议值，没有就「模型名」。
pub fn manual_placeholder(provider: &Provider) -> &'static str {
    provider.suggestion.unwrap_or("模型名")
}

/// 回退手输的帮助行（#26 拍板 2）：有建议值与没有建议值两句。
pub fn manual_help(provider: &Provider) -> &'static str {
    if provider.suggestion.is_some() {
        MANUAL_HELP_SUGGESTED
    } else {
        MANUAL_HELP_PLAIN
    }
}

/// 菜单公共帮助行。
pub const MENU_HELP: &str = "输入可筛选，回车确认";

/// 自定义 base_url 的帮助行。
pub const BASE_URL_HELP: &str = "OpenAI 兼容端点，含 /v1 之类的版本段";

/// base_url 为空。
pub const BASE_URL_EMPTY: &str = "base_url 不能为空";

/// base_url 不合形。
pub const BASE_URL_MALFORMED: &str = "需以 http:// 或 https:// 开头，并带主机名";

/// API Key 为空（非本地、无当前值时）。
pub const KEY_EMPTY: &str = "API Key 不能为空";

/// 模型名为空。
pub const MODEL_EMPTY: &str = "模型名不能为空";

/// 模型菜单末项。
pub const MODEL_MANUAL_ROW: &str = "手动输入模型名…";

/// 本地 Ollama 的 key 帮助行。
pub const OLLAMA_KEY_HELP: &str = "本地 Ollama 无需密钥，直接回车（写入占位值 ollama）";

/// 本地 Ollama 空 key 写入的占位值。
pub const OLLAMA_PLACEHOLDER: &str = "ollama";

/// 拉模型列表的进度行。
pub const MODELS_PROGRESS: &str = "正在获取模型列表…";

/// 连通性验证的进度行。
pub const VERIFY_PROGRESS: &str = "正在验证连通性…";

/// 回退手输（有建议值）的帮助行。
pub const MANUAL_HELP_SUGGESTED: &str = "未能获取模型列表，请手动输入（占位符为建议值）";

/// 回退手输（无建议值）的帮助行。
pub const MANUAL_HELP_PLAIN: &str = "未能获取模型列表，请手动输入模型名（咨询你的提供商）";

/// 429 的报错（登录向导唯二直接退出的限流 / 服务端失败之一）。
pub const RATE_LIMITED: &str = "错误：请求过于频繁（429），稍后重跑 bit login。";

/// 2xx 但信封不可解析；#26 表未列，按「服务端给了非预期内容」同一形状收场。
pub const INVALID_RESPONSE: &str = "错误：响应不可解析，稍后重跑 bit login。";

/// 重配时顶部一行的上下文（#26 拍板 4）。
pub fn current_config_line(key: &str, model: &str) -> String {
    format!("当前配置：{} / {model}", display_name(key))
}

/// 重配时 key 的帮助行：首 3 尾 4 掩码 + 留空保持。
pub fn key_keep_help(key: &str) -> String {
    format!("已配置（{}），留空保持不变", mask(key))
}

/// 成功摘要第一行。
pub fn saved_summary(key: &str, model: &str) -> String {
    format!("已保存 AI 供给：{} / {model}", display_name(key))
}

/// 成功摘要第二行。
pub fn endpoint_line(base_url: &str) -> String {
    format!("端点：{base_url}")
}

/// 成功摘要第三行。
pub fn config_file_line(path: &Path) -> String {
    format!("配置文件：{}", path.display())
}

/// 认证失败（401/403）：回 key 输入。
pub fn auth_failed(status: u16) -> String {
    format!("认证失败（{status}）：API Key 无效或已过期，请重新输入。")
}

/// 模型 / 参数类 4xx（含 404）：回模型输入。
pub fn model_unavailable(status: u16, model: &str) -> String {
    format!("模型不可用（{status}）：{model}。请换一个模型。")
}

/// 连不上 / 超时：两行，第二行给下一步。
pub fn network_error(base_url: &str) -> String {
    format!("错误：连不上 {base_url}（无法建立连接）。\n检查网络或代理设置，稍后重跑 bit login。")
}

/// 5xx：服务端错误。
pub fn server_error(code: u16) -> String {
    format!("错误：服务端错误（{code}），稍后重跑 bit login。")
}

/// L1：启动即读到坏配置，不静默覆盖。
pub fn config_unavailable(reason: &str, path: Option<&Path>) -> String {
    match path {
        Some(path) => format!(
            "错误：配置文件不可用：{}（{reason}）。修好或删除后重跑 bit login。",
            path.display()
        ),
        None => format!("错误：配置文件不可用（{reason}）。修好或删除后重跑 bit login。"),
    }
}

/// L2：验证通过但写盘失败。
pub fn write_failed(error: &std::io::Error, path: &Path) -> String {
    format!("错误：无法写入配置文件（{error}）：{}。", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_follow_the_frozen_menu() {
        assert_eq!(
            PROVIDERS.map(|provider| provider.key),
            crate::config::PROVIDERS
        );
        assert_eq!(
            PROVIDERS.map(|provider| provider.name),
            [
                "DeepSeek",
                "Kimi（Moonshot）",
                "智谱 GLM",
                "通义千问",
                "OpenAI",
                "OpenRouter",
                "SiliconFlow",
                "Ollama（本地）",
                CUSTOM_NAME,
            ]
        );
        assert_eq!(
            PROVIDERS.map(|provider| provider.hint),
            [
                "国内直连",
                "国内直连",
                "国内直连",
                "国内直连",
                "官方 API",
                "多模型聚合",
                "国内直连",
                "本机运行，免密钥",
                "手输 base_url",
            ]
        );
        assert_eq!(
            PROVIDERS.map(|provider| provider.base_url),
            [
                "https://api.deepseek.com",
                "https://api.moonshot.cn/v1",
                "https://open.bigmodel.cn/api/paas/v4/",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "https://api.openai.com/v1",
                "https://openrouter.ai/api/v1",
                "https://api.siliconflow.cn/v1",
                "http://localhost:11434/v1",
                "",
            ]
        );
        assert_eq!(
            PROVIDERS.map(|provider| provider.suggestion),
            [
                None,
                None,
                Some("GLM-5.3"),
                Some("qwen3.8-max"),
                None,
                None,
                Some("deepseek-ai/DeepSeek-V4-Flash"),
                None,
                None,
            ]
        );
        assert_eq!(provider("custom"), PROVIDERS.last());
        assert_eq!(display_name("deepseek"), "DeepSeek");
        assert_eq!(display_name("不认识的键"), CUSTOM_NAME);
    }

    #[test]
    fn mask_hides_the_middle() {
        assert_eq!(mask("sk-proj-abc12347890xyz"), "sk-…0xyz");
        assert_eq!(mask("123456789"), "123…6789");
        assert_eq!(mask("12345678"), "••••");
        assert_eq!(mask("short"), "••••");
        assert_eq!(mask(""), "••••");
    }

    #[test]
    fn base_url_validation_matches_the_frozen_copies() {
        assert_eq!(validate_base_url(""), Err(BASE_URL_EMPTY));
        assert_eq!(validate_base_url("   "), Err(BASE_URL_EMPTY));
        assert_eq!(
            validate_base_url("api.example.com"),
            Err(BASE_URL_MALFORMED)
        );
        assert_eq!(
            validate_base_url("ftp://api.example.com"),
            Err(BASE_URL_MALFORMED)
        );
        assert_eq!(validate_base_url("https://"), Err(BASE_URL_MALFORMED));
        assert_eq!(validate_base_url("http://"), Err(BASE_URL_MALFORMED));
        assert_eq!(validate_base_url("  https://api.example.com/v1  "), Ok(()));
        assert_eq!(validate_base_url("http://127.0.0.1:8080"), Ok(()));
    }

    #[test]
    fn fallback_copy_depends_on_whether_a_suggestion_exists() {
        let zhipu = provider("zhipu").expect("预置家");
        let custom = provider("custom").expect("自定义家");
        assert_eq!(manual_placeholder(zhipu), "GLM-5.3");
        assert_eq!(manual_placeholder(custom), "模型名");
        assert_eq!(manual_help(zhipu), MANUAL_HELP_SUGGESTED);
        assert_eq!(manual_help(custom), MANUAL_HELP_PLAIN);
    }

    #[test]
    fn dynamic_copy_matches_the_frozen_table() {
        assert_eq!(
            current_config_line("deepseek", "deepseek-v4-pro"),
            "当前配置：DeepSeek / deepseek-v4-pro"
        );
        assert_eq!(
            key_keep_help("sk-proj-abc12347890xyz"),
            "已配置（sk-…0xyz），留空保持不变"
        );
        assert_eq!(
            saved_summary("custom", "stub-model"),
            "已保存 AI 供给：自定义 OpenAI 兼容 / stub-model"
        );
        assert_eq!(
            endpoint_line("https://api.deepseek.com"),
            "端点：https://api.deepseek.com"
        );
        assert_eq!(
            config_file_line(Path::new("/tmp/bit/config.toml")),
            "配置文件：/tmp/bit/config.toml"
        );
        assert_eq!(
            auth_failed(401),
            "认证失败（401）：API Key 无效或已过期，请重新输入。"
        );
        assert_eq!(
            model_unavailable(404, "gpt-5"),
            "模型不可用（404）：gpt-5。请换一个模型。"
        );
        assert_eq!(
            network_error("https://api.deepseek.com"),
            "错误：连不上 https://api.deepseek.com（无法建立连接）。\n检查网络或代理设置，稍后重跑 bit login。"
        );
        assert_eq!(
            server_error(503),
            "错误：服务端错误（503），稍后重跑 bit login。"
        );
        assert_eq!(
            config_unavailable(
                "第 2 行：未知键 `x`",
                Some(Path::new("/tmp/bit/config.toml"))
            ),
            "错误：配置文件不可用：/tmp/bit/config.toml（第 2 行：未知键 `x`）。修好或删除后重跑 bit login。"
        );
        assert_eq!(
            config_unavailable("无法确定配置路径", None),
            "错误：配置文件不可用（无法确定配置路径）。修好或删除后重跑 bit login。"
        );
        let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "拒绝访问");
        assert_eq!(
            write_failed(&error, Path::new("/tmp/bit/config.toml")),
            "错误：无法写入配置文件（拒绝访问）：/tmp/bit/config.toml。"
        );
    }
}
