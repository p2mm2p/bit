//! 提交消息的领域数据与纯函数：11 类类型清单、编辑器种子、scope 校验、合格判定与预检映射。
//!
//! 行为细则冻结在 [行为 · bit commit 细则](https://github.com/p2mm2p/bit/issues/4)，
//! 校验责任在 bit、清洗委派给 git 的依据见
//! [ADR-0001](../../docs/adr/0001-bit-owns-the-editor-and-validation.md)，
//! 类型清单与语义来自 [采纳 · v0.1 类型清单](https://github.com/p2mm2p/bit/issues/9)
//! （commitlint `type-enum` 全集），文案逐字取自
//! [命令面](https://github.com/p2mm2p/bit/issues/8) 的冻结表（第 8、9 行）。
//! 本模块只放纯函数与数据（可单测），交互与 git 调用在 `flow`。

/// 提交类型 11 类；声明顺序即菜单顺序（#9 的清单顺序，`feat` / `fix` 置前）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitType {
    Feat,
    Fix,
    Docs,
    Style,
    Refactor,
    Perf,
    Test,
    Build,
    Ci,
    Chore,
    Revert,
}

impl CommitType {
    /// 菜单顺序：`feat → fix → docs → style → refactor → perf → test → build → ci → chore → revert`。
    pub const ALL: [CommitType; 11] = [
        CommitType::Feat,
        CommitType::Fix,
        CommitType::Docs,
        CommitType::Style,
        CommitType::Refactor,
        CommitType::Perf,
        CommitType::Test,
        CommitType::Build,
        CommitType::Ci,
        CommitType::Chore,
        CommitType::Revert,
    ];

    /// 类型名（commitlint `type-enum` 原文，全小写）。
    pub fn name(self) -> &'static str {
        match self {
            CommitType::Feat => "feat",
            CommitType::Fix => "fix",
            CommitType::Docs => "docs",
            CommitType::Style => "style",
            CommitType::Refactor => "refactor",
            CommitType::Perf => "perf",
            CommitType::Test => "test",
            CommitType::Build => "build",
            CommitType::Ci => "ci",
            CommitType::Chore => "chore",
            CommitType::Revert => "revert",
        }
    }

    /// 菜单里的一句中文语义（#9 的语义栏，逐字冻结）。
    pub fn hint(self) -> &'static str {
        match self {
            CommitType::Feat => "新功能",
            CommitType::Fix => "缺陷修复",
            CommitType::Docs => "文档",
            CommitType::Style => "格式，不影响语义",
            CommitType::Refactor => "重构",
            CommitType::Perf => "性能",
            CommitType::Test => "测试",
            CommitType::Build => "构建与依赖",
            CommitType::Ci => "CI 配置",
            CommitType::Chore => "杂项",
            CommitType::Revert => "回滚",
        }
    }
}

/// 菜单与模糊筛选都是对着这段文本做的。
impl std::fmt::Display for CommitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<10} {}", self.name(), self.hint())
    }
}

/// 编辑器种子的注释提示行（#8 表第 9 行，逐字冻结）；它随 `--cleanup=strip` 被剥掉、不进历史。
pub const COMMENT_HINT: &str = "# 描述必填；空行后写正文；引用 issue 用 Refs: #123";

/// scope 的 validator 报错（#4 第 6 步：这三个字符会破 header 文法，换行同理）。
pub const SCOPE_ERROR: &str = "不能含 ( ) : 或换行";

/// scope 归一化：trim 之后就定案——允许中文、不做大小写变换，只拒破文法的字符。
pub fn scope(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim();
    if trimmed
        .chars()
        .any(|ch| matches!(ch, '(' | ')' | ':' | '\n' | '\r'))
    {
        return Err(SCOPE_ERROR);
    }
    Ok(trimmed.to_string())
}

/// 编辑器种子的首行（#4 第 3 步）：`type(scope)!: ` —— scope 为空时整个省略括号，
/// breaking 时 `!` 紧贴冒号之前。
pub fn header(ty: CommitType, scope: &str, breaking: bool) -> String {
    let scope = scope.trim();
    let scope = if scope.is_empty() {
        String::new()
    } else {
        format!("({scope})")
    };
    let bang = if breaking { "!" } else { "" };
    format!("{}{scope}{bang}: ", ty.name())
}

/// 编辑器种子（#4 第 3 步）：预填首行 + 一行注释提示。行尾那个空格是预填的一部分，
/// 由 `--cleanup=strip` 在用户没补描述时抹掉——这正是空 subject 要被拦下的原因。
pub fn seed(ty: CommitType, scope: &str, breaking: bool) -> String {
    format!("{}\n{COMMENT_HINT}\n", header(ty, scope, breaking))
}

/// 编辑器返回后的合格判定（#4 第 4 步）：对 `git stripspace --strip-comments` 的输出
/// （= git 将保存的消息）判「首行含 `: ` 且其后有非空白内容」。
///
/// 实测（git 2.55.0.windows.5）：未编辑的种子经 `strip` 后是 `feat(ui):`（行尾空格被抹掉），
/// 于是这里判不合格、重开编辑器；只有规范 MUST 级的「描述非空」在这一档，
/// type 小写、subject 尾点、header ≤ 100 等风格规则不在 bit 的校验域（ADR-0001）。
pub fn is_acceptable(stripped: &str) -> bool {
    let Some(first_line) = stripped.lines().next() else {
        return false;
    };
    match first_line.split_once(": ") {
        Some((_, description)) => description.chars().any(|ch| !ch.is_whitespace()),
        None => false,
    }
}

/// 编辑器每一轮回来后的判决（#16 修订 #4 的「空则重开」：回环有界）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// 描述非空 → 交给 git 提交。
    Accept,
    /// 「git 将保存的消息」与上一轮逐字相同 → 用户没打算写，按取消收场。
    Cancel,
    /// 改动过但仍不合格 → 带着用户上次保存的原文重开编辑器。
    Reopen,
}

/// 逐轮判决：合格提交；与 `previous` 逐字相同 = 未改动即放弃；改动过也不合格才重开——
/// 循环因此有界，只有内容在变才继续。
///
/// `previous` 的初值是种子经 `git stripspace --strip-comments` 的结果：同一个尺子量种子与
/// 每一轮的产物（ADR-0001），「什么都没写就退出」于是与「写了又改回来」落到同一条判定上。
pub fn verdict(previous: &str, stripped: &str) -> Verdict {
    if is_acceptable(stripped) {
        Verdict::Accept
    } else if stripped == previous {
        Verdict::Cancel
    } else {
        Verdict::Reopen
    }
}

/// `git diff --cached --quiet` 的退出码 → bit 的下一步（#4 第 5 步、#8 表第 8 行）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Staged {
    /// 退出码 1：有暂存内容 → 继续交互。
    Present,
    /// 退出码 0：无暂存内容 → bit 拦下（stderr、退出码 1），不进交互。
    Missing,
    /// 其余：git 自身的失败（非仓库时是 129 的用法报错）→ 原样透传（ADR-0003）。
    Git,
}

/// 预检退出码的映射。
pub fn staged_check(code: i32) -> Staged {
    match code {
        1 => Staged::Present,
        0 => Staged::Missing,
        _ => Staged::Git,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_order_and_hints_are_frozen() {
        assert_eq!(
            CommitType::ALL.map(CommitType::name),
            [
                "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore",
                "revert"
            ]
        );
        assert_eq!(
            CommitType::ALL.map(CommitType::hint),
            [
                "新功能",
                "缺陷修复",
                "文档",
                "格式，不影响语义",
                "重构",
                "性能",
                "测试",
                "构建与依赖",
                "CI 配置",
                "杂项",
                "回滚"
            ]
        );
        assert_eq!(
            CommitType::Feat.to_string(),
            "feat       新功能",
            "菜单里是「类型名 + 一句语义」，供模糊筛选"
        );
    }

    #[test]
    fn header_follows_the_prefill_rule() {
        assert_eq!(header(CommitType::Feat, "", false), "feat: ");
        assert_eq!(header(CommitType::Feat, "ui", false), "feat(ui): ");
        assert_eq!(header(CommitType::Feat, "ui", true), "feat(ui)!: ");
        assert_eq!(header(CommitType::Fix, "", true), "fix!: ");
        assert_eq!(
            header(CommitType::Feat, "  ui  ", false),
            "feat(ui): ",
            "scope 已 trim"
        );
        assert_eq!(header(CommitType::Docs, "登录页", false), "docs(登录页): ");
    }

    #[test]
    fn seed_is_the_header_plus_one_comment_hint() {
        assert_eq!(
            seed(CommitType::Feat, "ui", false),
            format!("feat(ui): \n{COMMENT_HINT}\n")
        );
        assert_eq!(
            COMMENT_HINT,
            "# 描述必填；空行后写正文；引用 issue 用 Refs: #123"
        );
        let stripped = seed(CommitType::Feat, "ui", false);
        let mut lines = stripped.lines();
        assert_eq!(lines.next(), Some("feat(ui): "));
        assert!(lines.next().expect("有第二行").starts_with('#'));
        assert_eq!(lines.next(), None, "种子只有两行");
    }

    #[test]
    fn scope_allows_chinese_but_rejects_grammar_breakers() {
        assert_eq!(scope("").as_deref(), Ok(""));
        assert_eq!(scope("   ").as_deref(), Ok(""));
        assert_eq!(scope("  ui  ").as_deref(), Ok("ui"));
        assert_eq!(scope("登录页").as_deref(), Ok("登录页"));
        assert_eq!(scope("UI").as_deref(), Ok("UI"), "不做大小写变换");
        assert_eq!(scope("ui.list").as_deref(), Ok("ui.list"));
        for broken in ["feat(ui", "ui)", "feat: ui", "ui\nx", "ui\r\nx"] {
            assert_eq!(scope(broken), Err(SCOPE_ERROR), "输入 {broken:?}");
        }
        assert_eq!(SCOPE_ERROR, "不能含 ( ) : 或换行");
    }

    #[test]
    fn acceptability_requires_a_non_empty_description() {
        for acceptable in [
            "feat(ui): 加登录页\n",
            "feat(ui): 加登录页\n\n正文\n\nRefs: #123\n",
            "fix: fix a bug",
            "feat: x\r\n\r\nbody",
            "chore(deps)!: 换掉解析器",
            "fix: a: b",
        ] {
            assert!(is_acceptable(acceptable), "{acceptable:?} 应当合格");
        }
        for rejected in [
            "",
            "\n",
            "feat(ui):",
            "feat(ui): \n",
            "feat(ui): \n\n正文\n",
            "只有正文，没有冒号空格\n",
            "# 只剩注释\n",
        ] {
            assert!(!is_acceptable(rejected), "{rejected:?} 应当不合格");
        }
        assert!(
            is_acceptable("feat(ui):: 双冒号\n"),
            "只做 MUST 级的「描述非空」，header 文法不复核（ADR-0001）"
        );
    }

    #[test]
    fn verdict_bounds_the_loop_on_an_unchanged_message() {
        let seed = "feat(ui):";
        assert_eq!(verdict(seed, "feat(ui): 加登录页"), Verdict::Accept);
        assert_eq!(
            verdict(seed, seed),
            Verdict::Cancel,
            "什么都没写就退出编辑器 = 放弃，不在空描述上打转"
        );
        assert_eq!(
            verdict(seed, "只有正文，没有 header"),
            Verdict::Reopen,
            "改动过 → 带着用户上次保存的原文重开"
        );
        assert_eq!(
            verdict("只有正文，没有 header", "只有正文，没有 header"),
            Verdict::Cancel,
            "重开后又没改动 = 放弃"
        );
        assert_eq!(verdict("只有正文", "fix: 补上了"), Verdict::Accept);
    }

    #[test]
    fn staged_check_maps_the_three_verdicts() {
        assert_eq!(staged_check(1), Staged::Present);
        assert_eq!(staged_check(0), Staged::Missing);
        for git_failure in [128, 129, 2] {
            assert_eq!(
                staged_check(git_failure),
                Staged::Git,
                "退出码 {git_failure}"
            );
        }
    }
}
