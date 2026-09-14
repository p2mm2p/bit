# 领域文档

工程技能在探索本仓库代码时，应如何消费本仓库的领域文档。

## 探索之前，先读这些

- 仓库根目录的 **`CONTEXT.md`**；或
- 若根目录存在 **`CONTEXT-MAP.md`**：它指向每个上下文各一份 `CONTEXT.md`。按需读取与当前主题相关的每一份。
- **`docs/adr/`**：读取与你即将改动的区域相关的 ADR。多上下文仓库中，还要检查 `src/<context>/docs/adr/` 里的上下文级决策。

如果这些文件不存在，**静默继续**。不要点出它们缺失，也不要建议提前创建。`/domain-modeling` 技能（经 `/grill-with-docs` 与 `/improve-codebase-architecture` 触达）会在术语或决策真正落定时惰性创建它们。

## 文件结构

单上下文仓库（大多数仓库，本仓库属于此类）：

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

多上下文仓库（根目录存在 `CONTEXT-MAP.md`）：

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          ← 系统级决策
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← 上下文级决策
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 使用术语表的词汇

当你的产出提到某个领域概念（issue 标题、重构提案、某个假设、测试名）时，使用 `CONTEXT.md` 中定义的术语。不要漂移到术语表明确回避的同义词。

如果你需要的概念还不在术语表中，这本身是信号：要么你在发明项目并不使用的语言（重新考虑），要么存在真实缺口（记下来交给 `/domain-modeling`）。

## 标出 ADR 冲突

若你的产出与既有 ADR 矛盾，明确说出来，而不是悄悄覆盖：

> _与 ADR-0007（event-sourced orders）矛盾，但值得重新打开，因为……_
