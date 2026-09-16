//! 分支名的领域数据与纯函数：7 类类型清单、五步静默规范化、前缀剥离、文法复核与组装。
//!
//! 规范级校验责任在 bit（git 的 ref 规则比规范宽松得多，见
//! [ADR-0002](../../docs/adr/0002-bit-validates-branch-names.md)），行为细则冻结在
//! [行为 · bit branch 细则](https://github.com/p2mm2p/bit/issues/7)，
//! 类型清单来自 [采纳 · v0.1 类型清单](https://github.com/p2mm2p/bit/issues/9)，
//! 菜单语义与 validator 文案逐字取自 [命令面](https://github.com/p2mm2p/bit/issues/8) 的冻结表。
//! 本模块只放纯函数与数据（可单测），交互与 git 调用在 `flow`。

/// 分支类型 7 类；声明顺序即菜单顺序（#7、#9）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchType {
    Feature,
    Fix,
    Hotfix,
    Release,
    Chore,
    Docs,
    Test,
}

impl BranchType {
    /// 菜单顺序：`feature → fix → hotfix → release → chore → docs → test`。
    pub const ALL: [BranchType; 7] = [
        BranchType::Feature,
        BranchType::Fix,
        BranchType::Hotfix,
        BranchType::Release,
        BranchType::Chore,
        BranchType::Docs,
        BranchType::Test,
    ];

    /// 前缀名（`feature` 是正名，`feat` / `bugfix` 等别名不在内置清单内）。
    pub fn name(self) -> &'static str {
        match self {
            BranchType::Feature => "feature",
            BranchType::Fix => "fix",
            BranchType::Hotfix => "hotfix",
            BranchType::Release => "release",
            BranchType::Chore => "chore",
            BranchType::Docs => "docs",
            BranchType::Test => "test",
        }
    }

    /// 菜单里的一句中文语义（#8 表第 10 行，逐字冻结）。
    pub fn hint(self) -> &'static str {
        match self {
            BranchType::Feature => "新增功能",
            BranchType::Fix => "修复缺陷",
            BranchType::Hotfix => "线上紧急修复",
            BranchType::Release => "准备发布",
            BranchType::Chore => "非代码任务：依赖、文档、配置",
            BranchType::Docs => "仅文档改动（扩展类型）",
            BranchType::Test => "仅测试改动（扩展类型）",
        }
    }
}

/// 菜单与模糊筛选都是对着这段文本做的。
impl std::fmt::Display for BranchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<9} {}", self.name(), self.hint())
    }
}

/// bit 自己承担的校验失败（#8 表第 12 行的三类，逐字冻结）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchError {
    /// 白名单（`a–z 0–9 - .`）之外的字符，含中文。
    IllegalChar,
    /// 规范化之后描述段为空。
    Empty,
    /// 输入带了类型前缀，但与当前选中的类型不同。
    PrefixConflict,
}

impl BranchError {
    /// validator 原地显示的那句话。
    pub fn message(self) -> &'static str {
        match self {
            BranchError::IllegalChar => "只允许 a–z 0–9 - .",
            BranchError::Empty => "规范化后为空，请重新输入",
            BranchError::PrefixConflict => "前缀与所选类型不一致",
        }
    }
}

/// 一次输入定案之后的结果（#7 流程第 3–5 步）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// 交给 `git switch -c` 的最终名。
    pub name: String,
    /// 描述段（规范化与前缀剥离之后）。
    pub description: String,
    /// 输入与描述段不一致（发生过规范化或前缀剥离）→ 确认步骤回显原文。
    pub changed: bool,
}

/// 五步静默规范化（#7 → 研究笔记 §3）：
/// trim → 转小写 → 空格/下划线转 `-` → 折叠连续的 `-`/`.` → 去首尾 `-`/`.`。
///
/// 白名单外的字符原样保留（含中文），交给 [`BranchError::IllegalChar`] 报错，
/// 不做静默丢弃——用户得知道自己写了什么。
pub fn normalize(input: &str) -> String {
    let mut out = String::new();
    let mut ends_with_separator = false;
    for ch in input.trim().chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            ends_with_separator = false;
        } else if ch.is_whitespace() || ch == '_' || ch == '-' || ch == '.' {
            if !out.is_empty() && !ends_with_separator {
                out.push(if ch == '.' { '.' } else { '-' });
                ends_with_separator = true;
            }
        } else {
            out.push(ch);
            ends_with_separator = false;
        }
    }
    out.trim_end_matches(['-', '.']).to_string()
}

/// 输入 → 最终分支名；失败即 validator 原地报错重输的那三类。
pub fn resolve(raw: &str, selected: BranchType) -> Result<Resolved, BranchError> {
    let normalized = normalize(raw);
    let description = description_of(&normalized, selected)?;
    let changed = description != raw;
    Ok(Resolved {
        name: assemble(selected, &description),
        description,
        changed,
    })
}

/// 组装最终名：`<type>/<描述>`（#7 第 4 步）。
pub fn assemble(selected: BranchType, description: &str) -> String {
    format!("{}/{}", selected.name(), description)
}

/// 分支名的文法复核：`<type>/<desc>`，desc 是 `-` 分隔的若干段、每段由 `.` 分隔字母数字块
/// （Conventional Branch 1.1.0 的 `spec.json` 正则，这里手写等价检查，不引正则依赖）。
///
/// [`resolve`] 的产物必然通过这道复核——它是规范化算法的不变量，单测里显式钉住。
pub fn is_valid_name(name: &str) -> bool {
    let Some((prefix, description)) = name.split_once('/') else {
        return false;
    };
    if !BranchType::ALL.iter().any(|ty| ty.name() == prefix) {
        return false;
    }
    let mut inside_chunk = false;
    for ch in description.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            inside_chunk = true;
        } else if (ch == '-' || ch == '.') && inside_chunk {
            inside_chunk = false;
        } else {
            return false;
        }
    }
    inside_chunk
}

/// 前缀剥离（#7）：同类型前缀静默剥离，异类型前缀报错重输，其余原样留在描述段里
/// （其中的 `/` 会落到非法字符那一类）。
fn description_of(normalized: &str, selected: BranchType) -> Result<String, BranchError> {
    let description = match normalized.split_once('/') {
        Some((prefix, rest)) if BranchType::ALL.iter().any(|ty| ty.name() == prefix) => {
            if prefix != selected.name() {
                return Err(BranchError::PrefixConflict);
            }
            rest
        }
        _ => normalized,
    };
    let description = normalize(description);
    if description.is_empty() {
        return Err(BranchError::Empty);
    }
    if !description.chars().all(is_allowed_char) {
        return Err(BranchError::IllegalChar);
    }
    Ok(description)
}

fn is_allowed_char(ch: char) -> bool {
    ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '.'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_order_and_hints_are_frozen() {
        assert_eq!(
            BranchType::ALL.map(BranchType::name),
            [
                "feature", "fix", "hotfix", "release", "chore", "docs", "test"
            ]
        );
        assert_eq!(
            BranchType::ALL.map(BranchType::hint),
            [
                "新增功能",
                "修复缺陷",
                "线上紧急修复",
                "准备发布",
                "非代码任务：依赖、文档、配置",
                "仅文档改动（扩展类型）",
                "仅测试改动（扩展类型）"
            ]
        );
        assert_eq!(
            BranchType::Feature.to_string(),
            "feature   新增功能",
            "菜单里是「前缀 + 一句语义」，供模糊筛选"
        );
    }

    #[test]
    fn normalize_follows_the_five_steps() {
        for (input, expected) in [
            ("  add-login  ", "add-login"),
            ("Add OAuth Login", "add-oauth-login"),
            ("add_oauth_login", "add-oauth-login"),
            ("add--login", "add-login"),
            ("add..login", "add.login"),
            ("add-.login", "add-login"),
            ("-add.login-", "add.login"),
            ("v1.2.0", "v1.2.0"),
            ("add　login", "add-login"),
            ("", ""),
            ("---", ""),
            ("登录", "登录"),
            ("add 登录", "add-登录"),
        ] {
            assert_eq!(normalize(input), expected, "输入 {input:?}");
        }
    }

    #[test]
    fn resolve_normalizes_and_strips_the_matching_prefix() {
        assert_eq!(
            resolve("Add OAuth Login", BranchType::Feature),
            Ok(Resolved {
                name: "feature/add-oauth-login".to_string(),
                description: "add-oauth-login".to_string(),
                changed: true,
            })
        );
        assert_eq!(
            resolve("feature/add-oauth-login", BranchType::Feature),
            Ok(Resolved {
                name: "feature/add-oauth-login".to_string(),
                description: "add-oauth-login".to_string(),
                changed: true,
            }),
            "同类型前缀静默剥离"
        );
        assert_eq!(
            resolve("feature/-lead", BranchType::Feature),
            Ok(Resolved {
                name: "feature/lead".to_string(),
                description: "lead".to_string(),
                changed: true,
            }),
            "剥离后剩下的首尾分隔符也归规范化管"
        );
        assert_eq!(
            resolve("add-login", BranchType::Fix),
            Ok(Resolved {
                name: "fix/add-login".to_string(),
                description: "add-login".to_string(),
                changed: false,
            }),
            "原文即最终描述段时不回显规范化"
        );
    }

    #[test]
    fn validator_messages_match_the_frozen_table() {
        assert_eq!(
            resolve("feature/foo", BranchType::Fix),
            Err(BranchError::PrefixConflict)
        );
        assert_eq!(
            resolve("登录页", BranchType::Feature),
            Err(BranchError::IllegalChar)
        );
        assert_eq!(
            resolve("feat/foo", BranchType::Feature),
            Err(BranchError::IllegalChar)
        );
        assert_eq!(
            resolve("add login!", BranchType::Feature),
            Err(BranchError::IllegalChar)
        );
        assert_eq!(resolve("   ", BranchType::Feature), Err(BranchError::Empty));
        assert_eq!(
            resolve("feature/", BranchType::Feature),
            Err(BranchError::Empty)
        );
        assert_eq!(resolve("---", BranchType::Feature), Err(BranchError::Empty));

        assert_eq!(BranchError::IllegalChar.message(), "只允许 a–z 0–9 - .");
        assert_eq!(BranchError::Empty.message(), "规范化后为空，请重新输入");
        assert_eq!(
            BranchError::PrefixConflict.message(),
            "前缀与所选类型不一致"
        );
    }

    #[test]
    fn resolved_names_pass_the_grammar_review() {
        for raw in [
            "Add OAuth Login",
            "update ci config",
            "issue-123-patch",
            "v1.2.0",
            "cover_branch_flow",
            "  Trimmed  ",
        ] {
            for selected in BranchType::ALL {
                let resolved = resolve(raw, selected).expect("无前缀输入对任何类型都应当过关");
                assert!(
                    is_valid_name(&resolved.name),
                    "{raw:?}（选中 {}）产出 `{}` 不合文法",
                    selected.name(),
                    resolved.name
                );
            }
        }
        for raw in [
            "feature/add-oauth-login",
            "fix/header_bug",
            "hotfix/issue-123-patch",
            "release/v1.2.0",
            "chore/update-ci--config",
            "docs/update readme",
            "test/cover-branch-flow",
        ] {
            let selected = BranchType::ALL
                .into_iter()
                .find(|ty| raw.starts_with(&format!("{}/", ty.name())))
                .expect("这些用例都带类型前缀");
            let resolved = resolve(raw, selected).expect("带同类型前缀的输入应当过关");
            assert!(
                is_valid_name(&resolved.name),
                "{raw:?} 产出 `{}` 不合文法",
                resolved.name
            );
        }
    }

    #[test]
    fn grammar_review_matches_the_spec_examples() {
        for valid in [
            "feature/add-oauth-login",
            "feature/issue-123-new-login",
            "release/v1.2.0",
            "docs/update-readme",
        ] {
            assert!(is_valid_name(valid), "{valid} 应当合规");
        }
        for invalid in [
            "feature/UPPER",
            "feature/-lead",
            "feature/new--login",
            "feature/new_login",
            "feature/new..login",
            "feature/trailing-",
            "feature/",
            "feat/add-login",
            "add-login",
            "feature/add-login/extra",
        ] {
            assert!(!is_valid_name(invalid), "{invalid} 应当被判不合规");
        }
    }
}
