# Issue tracker: GitHub

本仓库的 issue 和 spec 使用 GitHub Issues 管理。所有相关操作默认使用 `gh` CLI。

## 基本约定

- **创建 issue**：`gh issue create --title "..." --body "..."`。多行正文建议使用 heredoc。
- **读取 issue**：`gh issue view <number> --comments`，并按需要读取评论和 labels。
- **列出 issues**：`gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`，按需要添加 `--label` 和 `--state` 过滤条件。
- **评论 issue**：`gh issue comment <number> --body "..."`
- **添加 / 移除 label**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **关闭 issue**：`gh issue close <number> --comment "..."`

仓库信息从 `git remote -v` 推断；在 clone 内执行时，`gh` 通常会自动识别当前 GitHub 仓库。

## Pull requests as a triage surface

**PRs as a request surface: no.** 如果本仓库将外部 PR 当作需求入口处理，可手动改为 `yes`；`/triage` 会读取这个标记。

当该值设为 `yes` 时，PR 会使用和 issue 相同的标签与状态流程，并使用对应的 `gh pr` 命令：

- **读取 PR**：`gh pr view <number> --comments`，并使用 `gh pr diff <number>` 查看 diff。
- **列出需要 triage 的外部 PR**：`gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments`，然后只保留 `authorAssociation` 为 `CONTRIBUTOR`、`FIRST_TIME_CONTRIBUTOR` 或 `NONE` 的 PR，排除 `OWNER`、`MEMBER`、`COLLABORATOR`。
- **评论 / 打标签 / 关闭 PR**：使用 `gh pr comment`、`gh pr edit --add-label` / `--remove-label`、`gh pr close`。

GitHub 的 issues 和 PRs 共用编号空间，所以 `#42` 可能是 issue，也可能是 PR。需要先用 `gh pr view 42` 判断，失败时再回退到 `gh issue view 42`。

## 当 skill 要求 “publish to the issue tracker”

创建一个 GitHub issue。

## 当 skill 要求 “fetch the relevant ticket”

运行 `gh issue view <number> --comments`。

## Wayfinding operations

供 `/wayfinder` 使用。**map** 是一个 GitHub issue，**child** issues 是具体 ticket。

- **Map**：一个带有 `wayfinder:map` label 的 issue，用来记录 Notes / Decisions-so-far / Fog 内容。使用 `gh issue create --label wayfinder:map` 创建。
- **Child ticket**：优先作为 map 的 GitHub sub-issue 创建。如果 sub-issues 不可用，则在 map 正文的任务列表中加入该 child，并在 child 正文顶部写入 `Part of #<map>`。Labels 使用 `wayfinder:<type>`，其中 `<type>` 可为 `research`、`prototype`、`grilling`、`task`。ticket 被认领后，分配给当前执行的开发者。
- **Blocking**：优先使用 GitHub 原生 issue dependencies。使用 `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>` 添加依赖，其中 `<blocker-db-id>` 是 blocker 的数字 database id，可通过 `gh api repos/<owner>/<repo>/issues/<n> --jq .id` 获取，不是 `#number` 或 `node_id`。如果 dependencies 不可用，则在 child 正文顶部使用 `Blocked by: #<n>, #<n>` 作为回退。所有 blocker 关闭后，该 ticket 视为不再阻塞。
- **Frontier query**：列出 map 下的 open children，排除仍有 open blocker 的 ticket 和已有 assignee 的 ticket；按 map 中顺序选择第一个。
- **Claim**：`gh issue edit <n> --add-assignee @me`，这是会话中的第一次写操作。
- **Resolve**：`gh issue comment <n> --body "<answer>"`，随后 `gh issue close <n>`，再把 context pointer 追加到 map 的 Decisions-so-far。
