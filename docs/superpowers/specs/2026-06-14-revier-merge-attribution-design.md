# Revier 多父 Merge 链路归因与 CLI 分离设计

## 背景

当前 Revier 的变更块归因以 `baseCommit..headCommit` 的最终文件 diff 为基础，再逐个提交读取 patch hunk 区间，最后用“提交触碰区间”和“最终变更块行号”做相交判断。这个模型在简单线性历史中可用，但在多人协作和大量 merge 的仓库中会出现明显偏差：

- merge commit 的 diff 输出可能不是普通 `@@ -x,y +a,b @@` hunk，现有解析器无法稳定解析。
- 原始提交可能早于当前筛选时间，但通过范围内 merge 进入最终净变化，导致提交集合中没有真正作者。
- 多人并行修改会造成行号漂移，提交发生时的行号和最终 diff block 行号不再可直接比较。
- rename 或 move 链路较长时，仅尝试当前路径和 `oldPath` 容易漏掉中间路径。

这些情况会让变更块的 `authors` 为空，前端因此显示“未知作者”。本次设计目标是让复杂 merge 链路中的块作者优先指向原始提交作者，并将分析逻辑从 Electron 显示层中拆出，方便构造 CLI 测试用例和后续性能演进。

## 目标

1. 变更块作者采用“原始提交作者优先”的口径。
2. 对通过多父 merge 进入目标分支的代码，尽量追踪到真正引入该变化的非 merge 提交。
3. 当无法使用 blame 得到可靠结果时，使用提交差异推断，并在 UI 上以叹号提示“归因可信度较低”。
4. 把分析核心拆成 Electron 无关模块，并提供 TypeScript CLI，方便独立运行、调试和构造复杂 Git fixture。
5. CLI 的命令形态按未来 Rust + clap 二进制可替换的方式设计，但本轮先使用 TypeScript 实现。

## 非目标

- 本轮不把分析核心重写为 Rust。
- 本轮不引入远程 Git 平台 API、认证、PR 信息或代码托管平台语义。
- 本轮不做语义级函数、类、AST 归因。
- 本轮不承诺对二进制文件、超大生成文件或 Git 无法 blame 的对象给出精确作者。
- 本轮不改变现有 UI 的整体布局，只补充归因可信度提示。

## 术语

- 原始提交：真正引入某段最终内容，或真正执行某个删除行为的非 merge 提交。
- merge 链路：从目标分支 merge commit 追溯到被合入分支原始提交的一组父子提交关系。
- 精确归因：基于 blame 或可验证父链追踪得到的归因结果。
- 推断归因：无法使用 blame 得到结果时，基于提交 patch、hunk 区间、路径和内容线索得到的结果。
- 局部归因：同一个变更块内部分行可以精确归因，部分行只能推断。

## 技术选型

### 本轮语言

分析核心继续使用 TypeScript。理由：

- 现有 diff、patch、IPC、类型定义和测试均为 TypeScript，迁移成本低。
- 本轮重点风险是算法正确性，不是语言运行时性能。
- 复杂 merge 归因的主要耗时预期来自 Git 子进程、blame 范围和历史扫描，优先应通过批量调用、缓存和 fixture 验证优化。

### CLI 风格

CLI 命令形态按 clap 风格设计：

- 使用明确的 subcommand。
- 使用长参数表达输入。
- 默认输出机器可读 JSON。
- 使用稳定退出码表达错误类别。

TypeScript 初版不绑定 Rust clap。本轮优先使用 Node 原生参数解析能力实现最小 CLI；如果实现阶段确认需要更完整的帮助信息或参数校验，再单独确认是否引入 TS CLI 解析依赖。未来迁移 Rust 时，Rust 版应保持同一命令和 JSON 契约。

### 未来 Rust 迁移边界

当大型仓库分析性能或独立分发成为明确瓶颈时，可将 `analysis-core` 替换为 Rust 实现，并使用 `clap` 构建 CLI。Electron 主进程通过稳定 JSON 契约调用 Rust 子进程，或在后续需要时再评估 N-API 集成。

## 架构设计

### 模块划分

新增 Electron 无关的分析核心：

```text
src/analysis-core/
  contracts/
    analysisRequest.ts
    analysisResult.ts
  git/
    gitClient.ts
    simpleGitAnalysisClient.ts
  diff/
    overlayBuilder.ts
  attribution/
    attributionService.ts
    blameOriginResolver.ts
    mergeOriginResolver.ts
    patchInferenceResolver.ts
  cache/
    analysisCache.ts

src/cli/
  revier-analysis.ts
```

现有主进程 IPC 调整为适配层：

```text
src/main/ipc/reviewIpc.ts
  -> 调用 analysis-core
  -> 不直接持有复杂归因算法
```

渲染进程继续只消费 `FileOverlay`、`DiffBlock`、`RelatedCommit` 等共享类型。

### 依赖方向

```text
CLI --------\
            -> analysis-core -> GitClient -> 本地 Git
Electron ---/

Renderer -> IPC -> Electron -> analysis-core
```

`analysis-core` 不依赖 Electron、Vue、Naive UI 或浏览器 API。

## 数据模型

在现有共享类型基础上扩展归因状态。

```ts
export type AttributionConfidence = 'precise' | 'inferred' | 'partial';

export type AttributionMethod = 'blame' | 'merge-trace' | 'patch-inference';

export interface AttributionWarning {
  code:
    | 'BLAME_UNAVAILABLE'
    | 'MERGE_TRACE_AMBIGUOUS'
    | 'PATH_HISTORY_INCOMPLETE'
    | 'DELETION_TRACE_INCOMPLETE';
  message: string;
}

export interface BlockAttributionSummary {
  confidence: AttributionConfidence;
  warnings: AttributionWarning[];
}

export interface RelatedCommitAttribution {
  method: AttributionMethod;
  viaMergeHashes: string[];
}
```

`DiffBlock` 增加：

```ts
attribution: BlockAttributionSummary;
```

`RelatedCommit` 增加：

```ts
attribution?: RelatedCommitAttribution;
```

兼容策略：

- 没有 `attribution` 的旧数据按 `precise` 以外的未知状态处理，渲染层不崩溃。
- 新分析结果必须写入 `attribution`。
- UI 不基于 `authors.length === 0` 推断错误原因，而基于 `attribution.warnings` 给出提示。

## 归因算法

### 总流程

1. 读取 `baseCommit` 和 `headCommit` 的文件内容，生成最终 side-by-side diff 与变更块。
2. 为每个变更块构建归因请求，包括文件路径、旧侧行区间、新侧行区间、变更类型和内容片段。
3. 优先使用 blame 追踪新侧最终内容来源。
4. 对 blame 返回的 merge commit 或边界提交，进入 merge 链路追踪。
5. 对删除块或 blame 不可用场景，使用 patch 推断。
6. 聚合作者、相关提交、merge 链路和归因可信度。

### 新增和修改块

对于包含新侧行的块：

1. 对 `headCommit` 的新侧行区间执行批量 blame。
2. blame 参数优先使用可追踪移动和复制的配置，例如 `-M`、`-C`，具体命令由 `GitClient` 封装。
3. 每个 blame 行得到一个来源提交。
4. 如果来源提交不是 merge commit，则作为原始提交候选。
5. 如果来源提交是 merge commit，则调用 `mergeOriginResolver` 继续追踪父链。
6. 同一块内多个来源提交按作者去重，保留相关提交列表。

### 删除块

删除块没有新侧最终行，不能直接依赖 `headCommit` blame。处理口径是：归因给真正执行删除行为的原始提交作者，而不是被删除内容在旧版本中的作者。

处理流程：

1. 使用旧侧行内容和旧侧行区间构造删除指纹。
2. 在 `baseCommit..headCommit` 的候选提交中扫描当前文件及已知历史路径的删除或替换 hunk。
3. 若候选提交是非 merge 提交，并且删除指纹匹配，则作为精确删除归因。
4. 若候选提交是 merge commit，则沿父链追踪到执行删除的非 merge 提交。
5. 若路径历史、内容指纹或父链存在歧义，则使用 patch 推断结果，并将块标记为 `inferred` 或 `partial`。

### Merge 链路追踪

`mergeOriginResolver` 接收来源 merge commit、文件路径、目标内容片段或删除指纹，递归检查父提交：

1. 读取 merge commit 的父提交列表。
2. 对新增和修改内容，判断目标内容是否已存在于某个父提交。
3. 若目标内容存在于某个父提交，继续在该父提交所在链路追踪。
4. 若目标内容不存在于任何父提交，但存在于 merge 结果，认为内容由 merge 冲突解决产生，归因给 merge commit 作者，并保持 `precise`。
5. 若多个父提交都包含等价内容，保留多个候选；无法唯一确定时标记 `partial`。
6. 为避免递归爆炸，对 `commitHash + filePath + contentFingerprint` 做缓存。

### Patch 推断

当 blame 或 merge 追踪无法给出完整结果时，使用当前 patch 区间逻辑的增强版：

1. 对普通提交解析 `@@` hunk。
2. 对 merge commit 显式按父提交展开普通 diff，避免直接解析 combined diff。
3. 使用路径候选集合处理 rename。
4. 输出作者和相关提交，同时把块标记为 `inferred` 或 `partial`。

UI 文案不得出现备用实现相关措辞，应表达为“归因可信度较低，已根据提交差异推断”。

## CLI 设计

### 命令

```bash
pnpm revier-analysis analyze --repo E:/repo/app --base <hash> --head <hash> --glob "src/**/*.ts" --format json
pnpm revier-analysis file-overlay --repo E:/repo/app --base <hash> --head <hash> --file src/app.ts --format json
pnpm revier-analysis trace-block --repo E:/repo/app --base <hash> --head <hash> --file src/app.ts --block-id block-1 --format json
```

未来 Rust 版保持同样形态：

```bash
revier-analysis analyze --repo ... --base ... --head ... --format json
```

### 参数

- `--repo`：本地 Git 仓库路径。
- `--base`：分析基准提交。
- `--head`：分析目标提交。
- `--file`：单文件 overlay 命令的目标路径。
- `--glob`：文件筛选规则，可重复传入。
- `--format`：输出格式，本轮只要求 `json`。
- `--pretty`：格式化 JSON，便于人工调试。
- `--explain`：输出归因链路说明，供测试和调试使用。

### 输出

CLI 输出稳定 JSON：

```json
{
  "version": 1,
  "range": {
    "baseCommit": "...",
    "headCommit": "..."
  },
  "files": [],
  "overlays": [],
  "warnings": []
}
```

`file-overlay` 输出单个 `FileOverlay`。`trace-block` 输出单个块的作者、相关提交、归因方法、merge 链路和警告。

### 退出码

- `0`：成功。
- `2`：参数错误。
- `3`：Git 命令失败。
- `4`：没有可分析的变更。
- `10`：分析内部错误。

错误输出写入 stderr，JSON 结果只写 stdout。

## UI 设计

### 作者标签

`DiffBlockAuthors` 接收块级 `attribution` 信息：

- `precise`：正常显示作者标签。
- `inferred`：作者标签旁显示叹号图标。
- `partial`：作者标签旁显示叹号图标。

tooltip 文案：

```text
归因可信度较低，已根据提交差异推断
```

如果有更具体的 warning，可在详情面板中显示更具体的中文说明，例如：

- 无法使用 blame 结果，已根据提交差异推断。
- 部分行存在多条可能的 merge 来源。
- 文件历史路径不完整，结果可能不完整。

### 详情面板

相关提交列表增加轻量来源说明：

- `blame` 或 `merge-trace` 结果不额外强调。
- `patch-inference` 结果显示同样的叹号提示。
- 若提交经由 merge 链路进入，可在详情中展示“经由 merge 提交 <shortHash> 引入”。

## 性能设计

1. 对同一文件同一 commit 的 blame 结果按行区间批量请求，避免逐行执行 Git。
2. 对 commit 元数据、父提交列表、merge 追踪结果和 patch 解析结果做任务级缓存。
3. 文件级分析可以并发，但默认并发数应有限，避免大型仓库中 Git 子进程过多。
4. CLI fixture 应包含性能样本，用来判断瓶颈在 Git 调用、解析、归因聚合还是 UI 渲染。
5. Rust 迁移只在性能证据明确后执行，避免提前重写。

## 测试策略

### 单元测试

- blame 输出解析。
- merge 父链追踪。
- patch 推断状态。
- `precise`、`inferred`、`partial` 聚合规则。
- CLI 参数解析和退出码。

### Git 集成测试

使用临时 Git 仓库构造以下场景：

- 线性历史中的新增、修改、删除。
- feature 分支原始提交早于筛选时间，通过范围内 merge 进入 head。
- merge conflict 中由 merge commit 首次写入内容。
- 多父 merge 中不同父提交包含相同内容，产生局部歧义。
- rename 后通过 merge 引入修改。
- 删除行为在 feature 分支发生，再通过 merge 进入目标分支。

### CLI 快照测试

每个复杂 Git fixture 使用 CLI 输出 JSON，并对关键字段做快照或结构断言：

- 作者列表。
- 相关提交 hash。
- `attribution.confidence`。
- `attribution.warnings`。
- `viaMergeHashes`。

### UI 测试

- `inferred` 块显示叹号图标。
- `partial` 块显示叹号图标。
- tooltip 文案不出现备用实现相关措辞。
- `precise` 块不显示额外警告。

## 开发环境配置

当前 Node 工具链继续使用 `fnm` 和 `pnpm`。

建议新增脚本：

```json
{
  "scripts": {
    "revier-analysis": "tsx src/cli/revier-analysis.ts",
    "test:analysis": "vitest run tests/unit tests/integration",
    "test:analysis:fixtures": "vitest run tests/integration"
  }
}
```

如果实现阶段发现项目尚未引入 `tsx` 或等价运行器，需要先提交依赖选择确认。也可以使用现有构建链路编译 CLI 后再执行，具体以实现计划阶段确认。

## 迁移步骤

1. 抽出 `analysis-core`，迁移现有 overlay、patch range 和 attribution 代码。
2. 定义新的归因数据类型，并保持旧 UI 消费路径兼容。
3. 增加 TypeScript CLI，先接通现有分析能力。
4. 增加复杂 merge Git fixture，复现当前“未知作者”问题。
5. 实现 blame 主归因。
6. 实现 merge 链路追踪。
7. 实现 patch 推断和叹号提示状态。
8. IPC 改为调用 `analysis-core`。
9. 补齐 UI 叹号提示和详情说明。
10. 使用 CLI 和现有测试验证结果。

## 验收标准

- 复杂 merge fixture 中，通过 merge 引入的最终新增或修改块显示原始非 merge 提交作者。
- 原始提交早于筛选时间，但通过范围内 merge 进入最终净变化时，仍能显示原始作者。
- blame 不可用时，块作者不再无解释地显示“未知作者”，而是显示叹号提示和可信度说明。
- UI 中不出现备用实现相关措辞。
- Electron 分析流程和 CLI 调用同一套 `analysis-core`。
- CLI 可以在不启动 Electron 的情况下输出单文件 overlay JSON。
- 现有简单线性历史归因测试继续通过。

## 风险与处理

- Git blame 对删除行不天然适用：删除块需要结合删除指纹和 patch 扫描，无法确认时标记为推断。
- merge 父链可能存在多个等价来源：保留多个候选并标记为局部归因，避免伪装成唯一答案。
- 大仓库 blame 成本高：以批量 blame、缓存和并发限制控制成本。
- rename 历史不完整：先使用当前路径、`oldPath` 和 Git rename 信息构造路径候选，无法完整追踪时产生 warning。
- Rust 迁移过早会增加重写成本：本轮只稳定 CLI 契约和 JSON schema，为后续替换保留接口。
