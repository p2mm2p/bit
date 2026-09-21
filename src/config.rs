//! AI 供给配置：路径解析、扁平 TOML 子集读写、`BIT_AI_*` 优先序与三态加载。
//!
//! 行为冻结在 [行为 · 配置存储与优先序](https://github.com/p2mm2p/bit/issues/25)：
//! 文件是手写的扁平四键子集（不引 `toml` crate）、`BIT_AI_*` 逐字段覆盖文件、
//! 路径 `BIT_CONFIG` > 平台默认、Unix 写盘 0600 + 原子替换。
//! 加载返回「未配置 / 配置错误 / 可用」三态，`Invalid` 的要点与路径供 `flow` 按
//! [命令面 · v0.2 增补](https://github.com/p2mm2p/bit/issues/27) 的冻结表渲染。
//! 本模块不读进程环境：`Env` 由边缘快照一次，函数显式吃参数，测试零系统副作用。

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 九种 `provider` 取值（#25 第 1 节）：8 家预置 + `custom`。
pub const PROVIDERS: [&str; 9] = [
    "deepseek",
    "kimi",
    "zhipu",
    "dashscope",
    "openai",
    "openrouter",
    "siliconflow",
    "ollama",
    "custom",
];

/// `provider` 全缺时的缺省值：能力表按 `custom` 走提示词兜底（#25 第 3 节）。
pub const DEFAULT_PROVIDER: &str = "custom";

/// 环境变量快照。空串一律按「未设置」处理（#25 第 2、3 节）。
#[derive(Clone, Debug, Default)]
pub struct Env {
    vars: HashMap<OsString, OsString>,
}

impl Env {
    /// 读一次真实进程环境；只在 `main` / `flow` 边缘调用（#25 第 5 节）。
    pub fn from_process() -> Env {
        Env {
            vars: std::env::vars_os().collect(),
        }
    }

    /// 显式给一组键值，供测试与边缘注入。
    pub fn from_pairs<K, V, I>(pairs: I) -> Env
    where
        K: Into<OsString>,
        V: Into<OsString>,
        I: IntoIterator<Item = (K, V)>,
    {
        Env {
            vars: pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }

    /// 取一个变量；未设置或空串都返回 `None`。
    pub fn get(&self, key: &str) -> Option<&OsStr> {
        self.vars
            .get(OsStr::new(key))
            .map(OsString::as_os_str)
            .filter(|value| !value.is_empty())
    }
}

/// 合并定案的可用配置（#25 第 1、3 节）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiConfig {
    /// [`PROVIDERS`] 之一；文件与环境都缺时是 [`DEFAULT_PROVIDER`]。
    pub provider: String,
    /// 端点。预置家也落盘——文件自足，换代理或镜像可手改。
    pub base_url: String,
    /// 模型名。
    pub model: String,
    /// 明文密钥。
    pub api_key: String,
}

/// 读取三态（术语见 `CONTEXT.md`，行为见 #25 第 4 节）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Config {
    /// 未配置：无文件且无 `BIT_AI_*`。AI 能力不存在，v0.1 路径零回归。
    Missing,
    /// 配置错误：`reason` 是 #27 冻结表里的 `<要点>`；`path` 为 `None`
    /// 仅当平台环境缺失、路径都定不下来（`APPDATA` / `HOME` 未设置）。
    Invalid {
        reason: String,
        path: Option<PathBuf>,
    },
    /// 可用。
    Ready(AiConfig),
}

/// 配置文件路径：`BIT_CONFIG` 整条覆盖 > 平台默认（#25 第 2 节）。
pub fn path(env: &Env) -> Result<PathBuf, String> {
    if let Some(selected) = env.get("BIT_CONFIG") {
        return Ok(PathBuf::from(selected));
    }
    platform_path(env)
}

#[cfg(windows)]
fn platform_path(env: &Env) -> Result<PathBuf, String> {
    windows_path(env)
}

#[cfg(unix)]
fn platform_path(env: &Env) -> Result<PathBuf, String> {
    unix_path(env)
}

/// Windows：`%APPDATA%\bit\config.toml`；`APPDATA` 缺失或为空即报错，不猜 `USERPROFILE`。
///
/// 两个平台实现都在所有平台编译：路径逻辑不碰平台 API，跨平台单测因此能全量覆盖
/// （当前平台用哪个由 `platform_path` 编译期选择）。
#[allow(dead_code)]
fn windows_path(env: &Env) -> Result<PathBuf, String> {
    env.get("APPDATA").map_or_else(
        || Err("无法确定配置路径：APPDATA 未设置或为空".to_string()),
        |appdata| Ok(PathBuf::from(appdata).join("bit").join("config.toml")),
    )
}

/// Unix：`$XDG_CONFIG_HOME/bit/config.toml`（相对值按 XDG 规范忽略）→
/// 兜底 `$HOME/.config/bit/config.toml`；`HOME` 也缺即报错。
#[allow(dead_code)]
fn unix_path(env: &Env) -> Result<PathBuf, String> {
    if let Some(xdg) = env
        .get("XDG_CONFIG_HOME")
        .filter(|xdg| xdg.as_encoded_bytes().starts_with(b"/"))
    {
        return Ok(PathBuf::from(xdg).join("bit").join("config.toml"));
    }
    env.get("HOME").map_or_else(
        || Err("无法确定配置路径：HOME 未设置或为空".to_string()),
        |home| {
            Ok(PathBuf::from(home)
                .join(".config")
                .join("bit")
                .join("config.toml"))
        },
    )
}

/// 边缘一步：解析路径再 [`load`]；路径都定不下来也算配置错误（此时无路径可指）。
pub fn load_from(env: &Env) -> Config {
    match path(env) {
        Ok(path) => load(&path, env),
        Err(reason) => Config::Invalid { reason, path: None },
    }
}

/// 按 `BIT_AI_*` 优先序合并文件与环境，返回三态；路径显式传入（#25 第 3–5 节）。
///
/// 文件只要存在就解析——坏文件不被环境变量掩盖；纯环境变量也可作完整配置。
pub fn load(path: &Path, env: &Env) -> Config {
    let state = read_file(path);
    if matches!(state, FileState::Missing) && !has_any_ai_env(env) {
        return Config::Missing;
    }
    let fields = match state {
        FileState::Missing => FileFields::default(),
        FileState::Invalid(reason) => return invalid(reason, path),
        FileState::Parsed(fields) => fields,
    };

    let provider = merge(env, "BIT_AI_PROVIDER", fields.provider)
        .unwrap_or_else(|| DEFAULT_PROVIDER.to_string());
    if !PROVIDERS.contains(&provider.as_str()) {
        return invalid(format!("provider 取值不认识：`{provider}`"), path);
    }

    let base_url = merge(env, "BIT_AI_BASE_URL", fields.base_url);
    let model = merge(env, "BIT_AI_MODEL", fields.model);
    let api_key = merge(env, "BIT_AI_API_KEY", fields.api_key);
    let mut missing = Vec::new();
    if base_url.is_none() {
        missing.push("base_url");
    }
    if model.is_none() {
        missing.push("model");
    }
    if api_key.is_none() {
        missing.push("api_key");
    }
    if !missing.is_empty() {
        return invalid(format!("缺 {}", missing.join("、")), path);
    }

    Config::Ready(AiConfig {
        provider,
        base_url: base_url.expect("已排除缺项"),
        model: model.expect("已排除缺项"),
        api_key: api_key.expect("已排除缺项"),
    })
}

/// 写配置（`bit login` 用）：目录不存在即建；临时文件 0600（Unix）→ 原子 rename 覆盖，
/// Windows 沿 `%APPDATA%` 默认 ACL、不额外加固（#25 第 2 节）。
pub fn write(path: &Path, config: &AiConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = temp_path(path);
    let _ = fs::remove_file(&temp);
    let result = write_file(&temp, &render(config)).and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 固定模板：头两行注释 + 固定顺序四键（#25 第 1 节，逐字冻结）。
fn render(config: &AiConfig) -> String {
    format!(
        "# bit 的 AI 供给 —— 由 bit login 生成，可手工编辑。\n\
         # 密钥为明文，请勿把本文件提交进仓库。\n\
         provider = \"{}\"\n\
         base_url = \"{}\"\n\
         model = \"{}\"\n\
         api_key = \"{}\"\n",
        config.provider, config.base_url, config.model, config.api_key
    )
}

/// 同目录临时名：带 pid，避免并发写互相踩。
fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// 新文件独占创建；Unix 建时即 0600（#25 第 2 节），其余平台用默认 ACL。
#[cfg(unix)]
fn write_file(path: &Path, contents: &str) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_file(path: &Path, contents: &str) -> io::Result<()> {
    use std::io::Write;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

/// 文件读取的中间态。
enum FileState {
    /// 文件不存在——未必是错误（`BIT_AI_*` 可独立成配置）。
    Missing,
    /// 读不出来或解析失败，字符串是 #27 的 `<要点>`。
    Invalid(String),
    Parsed(FileFields),
}

/// 扁平四键；`None` = 未写或空串（空串按未设置处理）。
#[derive(Default)]
struct FileFields {
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
}

fn read_file(path: &Path) -> FileState {
    if path.is_dir() {
        return FileState::Invalid("路径是目录，不是文件".to_string());
    }
    match fs::read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => FileState::Missing,
        Err(error) => FileState::Invalid(match error.kind() {
            io::ErrorKind::PermissionDenied => "没有读取权限".to_string(),
            _ => format!("读取失败（{error}）"),
        }),
        Ok(bytes) => match String::from_utf8(bytes) {
            Err(_) => FileState::Invalid("文件不是 UTF-8 文本".to_string()),
            Ok(text) => parse(&text).map_or_else(FileState::Invalid, FileState::Parsed),
        },
    }
}

/// 手写扁平子集解析（#25 第 1 节）：`键 = "值"`（单双引号都认）、`#` 注释、空行；
/// 其余（缺 `=`、无引号、行尾多余内容、未知键、键重复）按行号报错。
fn parse(text: &str) -> Result<FileFields, String> {
    let mut fields = FileFields::default();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = strip_comment(raw).trim();
        if content.is_empty() {
            continue;
        }
        let Some((key, rest)) = content.split_once('=') else {
            return Err(format!("第 {line} 行：缺 `=`"));
        };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(is_key_char) {
            return Err(format!("第 {line} 行：键名不合法"));
        }
        let value = parse_value(rest.trim(), line)?;
        let slot = match key {
            "provider" => &mut fields.provider,
            "base_url" => &mut fields.base_url,
            "model" => &mut fields.model,
            "api_key" => &mut fields.api_key,
            other => return Err(format!("第 {line} 行：未知键 `{other}`")),
        };
        if slot.is_some() {
            return Err(format!("第 {line} 行：键重复（`{key}`）"));
        }
        if !value.is_empty() {
            *slot = Some(value);
        }
    }
    Ok(fields)
}

/// 裸键的字符集（TOML 裸键的子集，够用即可）。
fn is_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

/// 去掉行内 `#` 注释，但引号里的 `#` 原样保留。
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    for (index, ch) in line.char_indices() {
        match quote {
            Some(open) => {
                if ch == open {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '#' => return &line[..index],
            None => {}
        }
    }
    line
}

/// 解析 `"值"` 或 `'值'`（不做转义，扁平子集够用）；空串也合法，后续按未设置处理。
fn parse_value(text: &str, line: usize) -> Result<String, String> {
    let Some(open) = text.chars().next() else {
        return Err(format!("第 {line} 行：缺值"));
    };
    if open != '"' && open != '\'' {
        return Err(format!("第 {line} 行：值要用引号（\"…\" 或 '…'）"));
    }
    let rest = &text[open.len_utf8()..];
    let Some(end) = rest.find(open) else {
        return Err(format!("第 {line} 行：值缺收尾引号"));
    };
    if !rest[end + open.len_utf8()..].trim().is_empty() {
        return Err(format!("第 {line} 行：值后有多余内容"));
    }
    Ok(rest[..end].to_string())
}

/// 逐字段合并：`BIT_AI_*` > 文件。空串已在 [`Env::get`] 归为未设置。
fn merge(env: &Env, key: &str, file: Option<String>) -> Option<String> {
    env.get(key)
        .map(|value| value.to_string_lossy().into_owned())
        .or(file)
}

fn has_any_ai_env(env: &Env) -> bool {
    [
        "BIT_AI_PROVIDER",
        "BIT_AI_BASE_URL",
        "BIT_AI_MODEL",
        "BIT_AI_API_KEY",
    ]
    .iter()
    .any(|key| env.get(key).is_some())
}

fn invalid(reason: impl Into<String>, path: &Path) -> Config {
    Config::Invalid {
        reason: reason.into(),
        path: Some(path.to_path_buf()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("config.toml");
        (dir, path)
    }

    fn env(pairs: &[(&str, &str)]) -> Env {
        Env::from_pairs(pairs.iter().copied())
    }

    fn write_text(path: &Path, text: &str) {
        fs::write(path, text).expect("写测试文件");
    }

    fn ai() -> AiConfig {
        AiConfig {
            provider: "deepseek".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-v4-pro".to_string(),
            api_key: "sk-test".to_string(),
        }
    }

    fn invalid(config: Config) -> (String, Option<PathBuf>) {
        match config {
            Config::Invalid { reason, path } => (reason, path),
            other => panic!("应为配置错误，实际是 {other:?}"),
        }
    }

    #[test]
    fn no_file_and_no_env_is_missing() {
        let (_dir, path) = case();
        assert_eq!(load(&path, &Env::default()), Config::Missing);
    }

    #[test]
    fn load_from_resolves_bit_config_and_reports_missing() {
        let (_dir, path) = case();
        let env = env(&[("BIT_CONFIG", path.to_str().expect("测试路径是 UTF-8"))]);
        assert_eq!(load_from(&env), Config::Missing);
    }

    #[test]
    fn load_from_without_a_platform_directory_is_invalid_without_a_path() {
        let (_dir, path) = case();
        let _ = path;
        let (reason, reported) = invalid(load_from(&Env::default()));
        assert!(reported.is_none());
        #[cfg(unix)]
        assert!(reason.contains("HOME"), "{reason}");
        #[cfg(windows)]
        assert!(reason.contains("APPDATA"), "{reason}");
    }

    #[test]
    fn env_only_config_is_ready_and_provider_defaults_to_custom() {
        let (_dir, path) = case();
        let env = env(&[
            ("BIT_AI_BASE_URL", "https://api.openai.com/v1"),
            ("BIT_AI_MODEL", "gpt-5"),
            ("BIT_AI_API_KEY", "sk-env"),
        ]);
        assert_eq!(
            load(&path, &env),
            Config::Ready(AiConfig {
                provider: DEFAULT_PROVIDER.to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5".to_string(),
                api_key: "sk-env".to_string(),
            })
        );
    }

    #[test]
    fn empty_env_values_are_unset() {
        let (_dir, path) = case();
        let env = env(&[
            ("BIT_AI_PROVIDER", ""),
            ("BIT_AI_BASE_URL", ""),
            ("BIT_AI_MODEL", ""),
            ("BIT_AI_API_KEY", ""),
        ]);
        assert_eq!(load(&path, &env), Config::Missing);
    }

    #[test]
    fn partial_env_without_a_file_is_invalid() {
        let (_dir, path) = case();
        let (reason, reported) = invalid(load(&path, &env(&[("BIT_AI_API_KEY", "sk-x")])));
        assert_eq!(reason, "缺 base_url、model");
        assert_eq!(reported, Some(path));
    }

    #[test]
    fn file_subset_accepts_both_quotes_comments_and_blank_lines() {
        let (_dir, path) = case();
        write_text(
            &path,
            "# 顶层注释\n\nprovider = 'ollama' # 尾注释\n\
             base_url = \"http://127.0.0.1:11434/v1\"\n\
             model = \"qwen3\"\n\
             api_key = 'ollama'\n",
        );
        assert_eq!(
            load(&path, &Env::default()),
            Config::Ready(AiConfig {
                provider: "ollama".to_string(),
                base_url: "http://127.0.0.1:11434/v1".to_string(),
                model: "qwen3".to_string(),
                api_key: "ollama".to_string(),
            })
        );
    }

    #[test]
    fn hash_inside_quotes_is_part_of_the_value() {
        let (_dir, path) = case();
        write_text(
            &path,
            "base_url = \"https://example.com/v1#frag\"\nmodel = \"m\"\napi_key = \"k\"\n",
        );
        let Config::Ready(config) = load(&path, &Env::default()) else {
            panic!("应为可用配置");
        };
        assert_eq!(config.base_url, "https://example.com/v1#frag");
    }

    #[test]
    fn env_overrides_the_file_field_by_field() {
        let (_dir, path) = case();
        let base = ai();
        write(&path, &base).expect("写配置");

        assert_eq!(load(&path, &Env::default()), Config::Ready(base.clone()));

        let all = load(
            &path,
            &env(&[
                ("BIT_AI_PROVIDER", "openai"),
                ("BIT_AI_BASE_URL", "https://env.example.com"),
                ("BIT_AI_MODEL", "env-model"),
                ("BIT_AI_API_KEY", "env-key"),
            ]),
        );
        assert_eq!(
            all,
            Config::Ready(AiConfig {
                provider: "openai".to_string(),
                base_url: "https://env.example.com".to_string(),
                model: "env-model".to_string(),
                api_key: "env-key".to_string(),
            })
        );

        let cases = [
            (
                "BIT_AI_PROVIDER",
                "openai",
                AiConfig {
                    provider: "openai".to_string(),
                    ..base.clone()
                },
            ),
            (
                "BIT_AI_BASE_URL",
                "https://env.example.com",
                AiConfig {
                    base_url: "https://env.example.com".to_string(),
                    ..base.clone()
                },
            ),
            (
                "BIT_AI_MODEL",
                "env-model",
                AiConfig {
                    model: "env-model".to_string(),
                    ..base.clone()
                },
            ),
            (
                "BIT_AI_API_KEY",
                "env-key",
                AiConfig {
                    api_key: "env-key".to_string(),
                    ..base.clone()
                },
            ),
        ];
        for (key, value, expected) in cases {
            assert_eq!(
                load(&path, &env(&[(key, value)])),
                Config::Ready(expected),
                "{key}"
            );
        }
    }

    #[test]
    fn env_can_complete_a_partial_file() {
        let (_dir, path) = case();
        write_text(&path, "provider = \"openai\"\nmodel = \"gpt-5\"\n");
        let config = load(
            &path,
            &env(&[
                ("BIT_AI_BASE_URL", "https://api.openai.com/v1"),
                ("BIT_AI_API_KEY", "sk-x"),
            ]),
        );
        let Config::Ready(config) = config else {
            panic!("应为可用配置");
        };
        assert_eq!(config.provider, "openai");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.model, "gpt-5");
        assert_eq!(config.api_key, "sk-x");
    }

    #[test]
    fn half_config_is_invalid_and_names_every_missing_field() {
        let (_dir, path) = case();
        write_text(
            &path,
            "provider = \"deepseek\"\nbase_url = \"https://api.deepseek.com\"\n",
        );
        let (reason, reported) = invalid(load(&path, &Env::default()));
        assert_eq!(reason, "缺 model、api_key");
        assert_eq!(reported, Some(path));
    }

    #[test]
    fn an_existing_empty_file_is_a_config_error_not_missing() {
        let (_dir, path) = case();
        write_text(&path, "");
        let (reason, _) = invalid(load(&path, &Env::default()));
        assert!(reason.starts_with("缺 "), "{reason}");
    }

    #[test]
    fn a_broken_file_is_not_masked_by_env() {
        let (_dir, path) = case();
        write_text(&path, "base_url https://x\n");
        let complete = env(&[
            ("BIT_AI_PROVIDER", "openai"),
            ("BIT_AI_BASE_URL", "https://api.openai.com/v1"),
            ("BIT_AI_MODEL", "gpt-5"),
            ("BIT_AI_API_KEY", "sk-env"),
        ]);
        let (reason, _) = invalid(load(&path, &complete));
        assert!(reason.contains("第 1 行"), "{reason}");
    }

    #[test]
    fn unknown_key_is_named_with_its_line() {
        let (_dir, path) = case();
        write_text(&path, "provider = \"deepseek\"\napi_timeout = \"30\"\n");
        let (reason, _) = invalid(load(&path, &Env::default()));
        assert!(reason.contains("`api_timeout`"), "{reason}");
        assert!(reason.contains("第 2 行"), "{reason}");
    }

    #[test]
    fn duplicate_key_is_invalid() {
        let (_dir, path) = case();
        write_text(&path, "base_url = \"a\"\nbase_url = \"b\"\n");
        let (reason, _) = invalid(load(&path, &Env::default()));
        assert!(reason.contains("键重复"), "{reason}");
        assert!(reason.contains("第 2 行"), "{reason}");
    }

    #[test]
    fn broken_lines_are_rejected_with_their_line_number() {
        let cases = [
            ("base_url\n", "缺 `=`"),
            ("= \"x\"\n", "键名不合法"),
            ("base url = \"x\"\n", "键名不合法"),
            ("base_url =\n", "缺值"),
            ("base_url = https://x\n", "值要用引号"),
            ("base_url = \"https://x\n", "值缺收尾引号"),
            ("base_url = \"https://x\" oops\n", "值后有多余内容"),
        ];
        for (text, fragment) in cases {
            let (_dir, path) = case();
            write_text(&path, text);
            let (reason, _) = invalid(load(&path, &Env::default()));
            assert!(reason.starts_with("第 1 行："), "{text:?} → {reason}");
            assert!(reason.contains(fragment), "{text:?} → {reason}");
        }
    }

    #[test]
    fn unknown_provider_is_invalid_even_from_env() {
        let (_dir, path) = case();
        write(&path, &ai()).expect("写配置");
        let (reason, _) = invalid(load(&path, &env(&[("BIT_AI_PROVIDER", "acme")])));
        assert!(reason.contains("`acme`"), "{reason}");
    }

    #[test]
    fn unknown_provider_in_the_file_is_invalid() {
        let (_dir, path) = case();
        write_text(
            &path,
            "provider = \"acme\"\nbase_url = \"u\"\nmodel = \"m\"\napi_key = \"k\"\n",
        );
        let (reason, _) = invalid(load(&path, &Env::default()));
        assert!(reason.contains("`acme`"), "{reason}");
    }

    #[test]
    fn empty_provider_in_the_file_falls_back_to_custom() {
        let (_dir, path) = case();
        write_text(
            &path,
            "provider = \"\"\nbase_url = \"u\"\nmodel = \"m\"\napi_key = \"k\"\n",
        );
        let Config::Ready(config) = load(&path, &Env::default()) else {
            panic!("应为可用配置");
        };
        assert_eq!(config.provider, "custom");
    }

    #[test]
    fn non_utf8_file_is_invalid() {
        let (_dir, path) = case();
        fs::write(&path, [0xff, 0xfe, 0x00]).expect("写坏字节");
        let (reason, _) = invalid(load(&path, &Env::default()));
        assert_eq!(reason, "文件不是 UTF-8 文本");
    }

    #[test]
    fn a_directory_at_the_path_is_invalid() {
        let (_dir, path) = case();
        fs::create_dir(&path).expect("建同名目录");
        let (reason, _) = invalid(load(&path, &Env::default()));
        assert_eq!(reason, "路径是目录，不是文件");
    }

    #[test]
    fn bit_config_chooses_the_file() {
        let env = env(&[
            ("BIT_CONFIG", "/tmp/bit/config.toml"),
            ("APPDATA", "C:\\roaming"),
            ("XDG_CONFIG_HOME", "/xdg"),
            ("HOME", "/home/me"),
        ]);
        assert_eq!(path(&env), Ok(PathBuf::from("/tmp/bit/config.toml")));
    }

    #[test]
    fn empty_bit_config_is_ignored() {
        let env = env(&[("BIT_CONFIG", ""), ("APPDATA", "C:\\roaming")]);
        assert_eq!(
            windows_path(&env),
            Ok(PathBuf::from("C:\\roaming").join("bit").join("config.toml"))
        );
    }

    #[test]
    fn windows_path_needs_appdata() {
        let with_appdata = env(&[("APPDATA", "C:\\Users\\me\\AppData\\Roaming")]);
        assert_eq!(
            windows_path(&with_appdata),
            Ok(PathBuf::from("C:\\Users\\me\\AppData\\Roaming")
                .join("bit")
                .join("config.toml"))
        );
        let reason = windows_path(&Env::default()).expect_err("缺 APPDATA 应报错");
        assert!(reason.contains("APPDATA"), "{reason}");
        assert!(windows_path(&env(&[("APPDATA", "")])).is_err());
    }

    #[test]
    fn unix_path_prefers_absolute_xdg_then_home() {
        let xdg = env(&[("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/me")]);
        assert_eq!(
            unix_path(&xdg),
            Ok(PathBuf::from("/xdg").join("bit").join("config.toml"))
        );
        let home_only = env(&[("HOME", "/home/me")]);
        assert_eq!(
            unix_path(&home_only),
            Ok(PathBuf::from("/home/me")
                .join(".config")
                .join("bit")
                .join("config.toml"))
        );
    }

    #[test]
    fn unix_path_ignores_relative_xdg_and_needs_home() {
        let relative = env(&[("XDG_CONFIG_HOME", "relative/xdg"), ("HOME", "/home/me")]);
        assert_eq!(
            unix_path(&relative),
            Ok(PathBuf::from("/home/me")
                .join(".config")
                .join("bit")
                .join("config.toml"))
        );
        let reason = unix_path(&Env::default()).expect_err("缺 HOME 应报错");
        assert!(reason.contains("HOME"), "{reason}");
        assert!(unix_path(&env(&[("HOME", "")])).is_err());
        // 绝对 XDG 可用时 HOME 不再参与；HOME 为空也无妨
        assert_eq!(
            unix_path(&env(&[("XDG_CONFIG_HOME", "/xdg"), ("HOME", "")])),
            Ok(PathBuf::from("/xdg").join("bit").join("config.toml"))
        );
    }

    #[test]
    fn write_creates_directories_and_round_trips() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("nested").join("bit").join("config.toml");
        let config = ai();
        write(&path, &config).expect("写配置");
        assert_eq!(load(&path, &Env::default()), Config::Ready(config));

        let names: Vec<OsString> = fs::read_dir(path.parent().expect("有父目录"))
            .expect("读目录")
            .map(|entry| entry.expect("目录项").file_name())
            .collect();
        assert_eq!(names, vec![OsString::from("config.toml")]);
    }

    #[test]
    fn write_replaces_an_existing_file() {
        let (_dir, path) = case();
        write_text(
            &path,
            "provider = \"ollama\"\nbase_url = \"old\"\nmodel = \"old\"\napi_key = \"old\"\n",
        );
        let config = ai();
        write(&path, &config).expect("覆盖写");
        assert_eq!(load(&path, &Env::default()), Config::Ready(config));
    }

    #[test]
    fn write_reports_io_errors() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, "不是目录").expect("写阻塞文件");
        assert!(write(&blocker.join("config.toml"), &ai()).is_err());
    }

    #[test]
    fn write_template_is_frozen() {
        let (_dir, path) = case();
        write(&path, &ai()).expect("写配置");
        assert_eq!(
            fs::read_to_string(&path).expect("读配置"),
            "# bit 的 AI 供给 —— 由 bit login 生成，可手工编辑。\n\
             # 密钥为明文，请勿把本文件提交进仓库。\n\
             provider = \"deepseek\"\n\
             base_url = \"https://api.deepseek.com\"\n\
             model = \"deepseek-v4-pro\"\n\
             api_key = \"sk-test\"\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_creates_a_missing_file_as_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, path) = case();
        write(&path, &ai()).expect("写配置");
        let mode = fs::metadata(&path).expect("读元数据").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn write_tightens_an_existing_file_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, path) = case();
        write_text(&path, "provider = \"ollama\"\n");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("放宽权限");
        write(&path, &ai()).expect("覆盖写");
        let mode = fs::metadata(&path).expect("读元数据").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
