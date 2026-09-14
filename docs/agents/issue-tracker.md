# Issue 跟踪：GitHub

本仓库的 issue 与 spec 都存放在 GitHub Issues。所有操作使用 `gh` CLI。

## 约定

- **创建 issue**：`gh issue create --title "..." --body "..."`。多行正文用 heredoc。
- **读取 issue**：`gh issue view <number> --comments`，用 `jq` 过滤评论，并一并取回标签。
- **列出 issue**：`gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`，配合相应的 `--label` 与 `--state` 过滤。
- **评论 issue**：`gh issue comment <number> --body "..."`
- **打标签 / 去标签**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **关闭**：`gh issue close <number> --comment "..."`

仓库由 `git remote -v` 推断；在克隆目录内运行时 `gh` 会自动判断。

## PR 是否作为 triage 入口

**PR 作为请求入口：否。**（若本仓库把外部 PR 当作功能请求处理，改为 `是`；`/triage` 会读取此标志。）

设为 `是` 时，PR 走与 issue 相同的标签和状态，命令换成 `gh pr` 的对应形式：

- **读取 PR**：`gh pr view <number> --comments`，diff 用 `gh pr diff <number>`。
- **列出待 triage 的外部 PR**：`gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments`，只保留 `authorAssociation` 为 `CONTRIBUTOR`、`FIRST_TIME_CONTRIBUTOR`、`NONE` 的项（丢弃 `OWNER`/`MEMBER`/`COLLABORATOR`）。
- **评论 / 打标签 / 关闭**：`gh pr comment`、`gh pr edit --add-label`/`--remove-label`、`gh pr close`。

GitHub 的 issue 与 PR 共用同一个编号空间，因此裸写的 `#42` 可能指其中之一：先用 `gh pr view 42` 解析，失败再回退到 `gh issue view 42`。

## 当技能说 “publish to the issue tracker”（发布到 issue tracker）

创建一个 GitHub issue。

## 当技能说 “fetch the relevant ticket”（取出相关 ticket）

运行 `gh issue view <number> --comments`。

## Wayfinding 操作

供 `/wayfinder` 使用。**地图（map）** 是一个 issue，**子 ticket** 是它的子 issue。

- **地图**：单个带 `wayfinder:map` 标签的 issue，正文承载 Notes / Decisions-so-far / Fog。`gh issue create --label wayfinder:map`。
- **子 ticket**：作为 GitHub sub-issue 链接到地图（对 sub-issues 端点调用 `gh api`）。在未启用 sub-issues 的地方，改为把子 issue 加进地图正文的任务列表，并在子 issue 正文顶部写 `Part of #<map>`。标签：`wayfinder:<type>`（`research`/`prototype`/`grilling`/`task`）。被领取后，指派给驱动的开发者。
- **阻塞**：使用 GitHub **原生 issue dependencies**，这是规范且 UI 可见的表示。用 `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>` 添加边，其中 `<blocker-db-id>` 是阻塞者的数字**数据库 id**（`gh api repos/<owner>/<repo>/issues/<n> --jq .id`，**不是** `#number`，也不是 `node_id`）。GitHub 通过 `issue_dependencies_summary.blocked_by` 报告（只含未关闭的阻塞者，即实时闸门）。在 dependencies 不可用的地方，回退到子 issue 正文顶部的 `Blocked by: #<n>, #<n>` 行。所有阻塞者都关闭时，ticket 即解除阻塞。
- **边界查询（frontier query）**：列出地图下所有未关闭的子 issue（`gh issue list --state open`，限定到地图的 sub-issues / 任务列表），剔除任何存在未关闭阻塞者（`issue_dependencies_summary.blocked_by > 0`，或 `Blocked by` 行中有未关闭 issue）或已有 assignee 的项；按地图顺序取第一个。
- **领取**：`gh issue edit <n> --add-assignee @me`，这是本次会话的第一次写入。
- **解决**：`gh issue comment <n> --body "<answer>"`，然后 `gh issue close <n>`，最后把上下文指针（要点 + 链接）追加到地图的 Decisions-so-far。
