# Revier 多父 Merge 链路归因 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Revier 的变更块归因升级为“原始提交作者优先”的复杂 merge 链路归因，并提供可独立运行的 TypeScript CLI。

**Architecture:** 新增 Electron 无关的 `analysis-core`，由 Electron IPC 和 CLI 共同调用。归因算法以 blame 和 merge 父链追踪为主，无法得到可靠 blame 结果时使用提交差异推断，并把归因可信度传给 UI 显示叹号提示。

**Tech Stack:** TypeScript、Electron、Vue 3、Vitest、simple-git、Node 24、pnpm、fnm。本轮不引入 Rust，不新增 TS CLI 运行时依赖；CLI 使用 `tsc` 编译后由 Node 执行。

---

## 文件结构

- 创建：`src/analysis-core/contracts/analysisRequest.ts`
  - 定义 CLI 与 IPC 共享的分析请求结构。
- 创建：`src/analysis-core/contracts/analysisResult.ts`
  - 定义 CLI 输出 JSON 外层结构。
- 创建：`src/analysis-core/git/analysisGitClient.ts`
  - 定义分析核心依赖的 Git 能力接口与 blame 数据结构。
- 创建：`src/analysis-core/git/simpleGitAnalysisClient.ts`
  - 基于 `simple-git` 实现分析 Git 客户端。
- 创建：`src/analysis-core/git/blameParser.ts`
  - 解析 `git blame --line-porcelain` 输出。
- 创建：`src/analysis-core/cache/analysisCache.ts`
  - 提供任务级内存缓存，缓存 commit、父链、blame、patch 结果。
- 创建：`src/analysis-core/attribution/attributionService.ts`
  - 对外提供文件级变更块归因入口。
- 创建：`src/analysis-core/attribution/blameOriginResolver.ts`
  - 处理新增和修改块的新侧 blame 归因。
- 创建：`src/analysis-core/attribution/mergeOriginResolver.ts`
  - 沿 merge 父链追踪原始提交。
- 创建：`src/analysis-core/attribution/patchInferenceResolver.ts`
  - 在 blame 不可用时根据提交差异推断归因。
- 创建：`src/analysis-core/overlay/fileOverlayService.ts`
  - 从 repo、range、file 构建 `FileOverlay`。
- 创建：`src/analysis-core/index.ts`
  - 汇总导出核心 API。
- 创建：`src/cli/revier-analysis.ts`
  - TypeScript CLI 入口。
- 创建：`tsconfig.cli.json`
  - CLI 专用编译配置。
- 创建：`tests/unit/analysisCoreTypes.test.ts`
  - 验证归因类型和默认状态。
- 创建：`tests/unit/blameParser.test.ts`
  - 验证 blame porcelain 解析。
- 创建：`tests/unit/analysisCli.test.ts`
  - 验证 CLI 参数解析和错误输出。
- 创建：`tests/unit/attributionService.test.ts`
  - 验证精确、推断、局部归因聚合。
- 创建：`tests/integration/mergeAttribution.test.ts`
  - 用临时 Git 仓库验证复杂 merge 链路归因。
- 修改：`src/shared/reviewTypes.ts`
  - 增加归因可信度、归因方法、warning 和链路字段。
- 修改：`src/main/git/gitService.ts`
  - 复用 `SimpleGitAnalysisClient` 或补齐缺少的 Git 方法。
- 修改：`src/main/ipc/reviewIpc.ts`
  - IPC 改为调用 `analysis-core` 文件 overlay 服务。
- 修改：`src/renderer/components/review/DiffBlockAuthors.vue`
  - 根据归因可信度显示叹号图标和 tooltip。
- 修改：`src/renderer/components/review/DiffViewer.vue`
  - 把块级 `attribution` 传给作者组件。
- 修改：`src/renderer/components/review/BlockDetailPanel.vue`
  - 在相关提交里展示 merge 链路和可信度提示。
- 修改：`tests/fixtures/gitRepo.ts`
  - 增加 branch、checkout、merge、remove 文件等 Git fixture 工具。
- 修改：`tests/unit/reviewIpc.test.ts`
  - 调整 IPC 单元测试以适配新的 analysis-core 返回结构。
- 修改：`tests/unit/diffViewer.test.ts`
  - 验证作者组件收到归因状态。
- 修改：`tests/unit/blockDetailPanel.test.ts`
  - 验证详情面板展示可信度提示。
- 修改：`package.json`
  - 增加 CLI 编译和运行脚本。
- 修改：`tsconfig.node.json`
  - 纳入 `src/analysis-core/**/*.ts` 和 `src/cli/**/*.ts` 的类型检查。

## 任务 1：扩展共享归因类型

**Files:**
- Modify: `src/shared/reviewTypes.ts`
- Create: `tests/unit/analysisCoreTypes.test.ts`

- [ ] **Step 1: 写类型行为测试**

创建 `tests/unit/analysisCoreTypes.test.ts`：

```ts
import type { DiffBlock, RelatedCommit } from '../../src/shared/reviewTypes';

describe('analysis attribution shared types', () => {
  it('supports block attribution confidence and related commit provenance', () => {
    const relatedCommit: RelatedCommit = {
      hash: 'a'.repeat(40),
      shortHash: 'aaaaaaaa',
      authorName: 'Alice',
      authorEmail: 'alice@example.com',
      committedAt: '2026-06-01T00:00:00.000Z',
      subject: 'feat: 原始提交',
      matchedByFilter: false,
      touchedRanges: [],
      attribution: {
        method: 'merge-trace',
        viaMergeHashes: ['b'.repeat(40)]
      }
    };

    const block: DiffBlock = {
      id: 'block-1',
      oldStart: 1,
      oldEnd: 1,
      newStart: 1,
      newEnd: 1,
      changeType: 'modified',
      authors: [{ name: 'Alice', email: 'alice@example.com' }],
      rows: [],
      relatedCommits: [relatedCommit],
      attribution: {
        confidence: 'partial',
        warnings: [
          {
            code: 'MERGE_TRACE_AMBIGUOUS',
            message: '部分行存在多条可能的 merge 来源'
          }
        ]
      }
    };

    expect(block.attribution?.confidence).toBe('partial');
    expect(block.relatedCommits[0].attribution?.method).toBe('merge-trace');
    expect(block.relatedCommits[0].attribution?.viaMergeHashes).toHaveLength(1);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/analysisCoreTypes.test.ts
```

预期：失败，TypeScript 报告 `attribution`、`AttributionConfidence`、`AttributionMethod` 或 warning code 尚不存在。

- [ ] **Step 3: 增加共享类型**

修改 `src/shared/reviewTypes.ts`，在 `AuthorFilterOption` 后加入：

```ts
export type AttributionConfidence = 'precise' | 'inferred' | 'partial';

export type AttributionMethod = 'blame' | 'merge-trace' | 'patch-inference';

export type AttributionWarningCode =
  | 'BLAME_UNAVAILABLE'
  | 'MERGE_TRACE_AMBIGUOUS'
  | 'PATH_HISTORY_INCOMPLETE'
  | 'DELETION_TRACE_INCOMPLETE';

export interface AttributionWarning {
  code: AttributionWarningCode;
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

在 `RelatedCommit` 中增加：

```ts
  attribution?: RelatedCommitAttribution;
```

在 `DiffBlock` 中增加：

```ts
  attribution?: BlockAttributionSummary;
```

这里让 `DiffBlock.attribution` 保持可选，原因是现有测试 fixture 和提交下钻 overlay 仍会构造旧形态对象；后续 `analysis-core` 生成的新范围分析结果必须主动写入该字段。

- [ ] **Step 4: 运行类型测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/analysisCoreTypes.test.ts
```

预期：通过。

- [ ] **Step 5: 提交**

运行：

```bash
git add src/shared/reviewTypes.ts tests/unit/analysisCoreTypes.test.ts
git commit -m "feat: 添加归因可信度类型"
```

## 任务 2：创建 analysis-core 契约和缓存骨架

**Files:**
- Create: `src/analysis-core/contracts/analysisRequest.ts`
- Create: `src/analysis-core/contracts/analysisResult.ts`
- Create: `src/analysis-core/cache/analysisCache.ts`
- Create: `src/analysis-core/index.ts`
- Modify: `tsconfig.node.json`
- Test: `tests/unit/analysisCoreTypes.test.ts`

- [ ] **Step 1: 写核心契约导出测试**

追加到 `tests/unit/analysisCoreTypes.test.ts`：

```ts
import { createAnalysisCache } from '../../src/analysis-core';
import type { AnalyzeRequest, AnalysisCliResult } from '../../src/analysis-core';

describe('analysis-core contracts', () => {
  it('exports request, result, and cache contracts without Electron dependencies', () => {
    const request: AnalyzeRequest = {
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      globRules: ['src/**/*.ts']
    };

    const result: AnalysisCliResult = {
      version: 1,
      range: {
        baseCommit: request.baseCommit,
        headCommit: request.headCommit
      },
      files: [],
      overlays: [],
      warnings: []
    };

    const cache = createAnalysisCache();
    cache.parents.set('merge', ['left', 'right']);

    expect(result.version).toBe(1);
    expect(cache.parents.get('merge')).toEqual(['left', 'right']);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/analysisCoreTypes.test.ts
```

预期：失败，`src/analysis-core` 尚不存在。

- [ ] **Step 3: 创建请求契约**

创建 `src/analysis-core/contracts/analysisRequest.ts`：

```ts
export interface AnalyzeRequest {
  repoPath: string;
  baseCommit: string;
  headCommit: string;
  globRules: string[];
  format?: 'json';
  pretty?: boolean;
  explain?: boolean;
}

export interface FileOverlayAnalysisRequest {
  repoPath: string;
  baseCommit: string;
  headCommit: string;
  filePath: string;
  oldPath?: string;
  format?: 'json';
  pretty?: boolean;
  explain?: boolean;
}

export interface TraceBlockRequest extends FileOverlayAnalysisRequest {
  blockId: string;
}
```

- [ ] **Step 4: 创建结果契约**

创建 `src/analysis-core/contracts/analysisResult.ts`：

```ts
import type { AppError } from '../../shared/errors';
import type { ChangedFile, FileOverlay } from '../../shared/reviewTypes';

export interface AnalysisCliResult {
  version: 1;
  range: {
    baseCommit: string;
    headCommit: string;
  };
  files: ChangedFile[];
  overlays: FileOverlay[];
  warnings: AppError[];
}

export interface TraceBlockResult {
  version: 1;
  overlay: FileOverlay;
  blockId: string;
}
```

- [ ] **Step 5: 创建缓存结构**

创建 `src/analysis-core/cache/analysisCache.ts`：

```ts
import type { RelatedCommit, TouchedRange } from '../../shared/reviewTypes';
import type { BlameLine } from '../git/analysisGitClient';

export interface AnalysisCache {
  commits: Map<string, RelatedCommit>;
  parents: Map<string, string[]>;
  blameRanges: Map<string, BlameLine[]>;
  patches: Map<string, string>;
  touchedRanges: Map<string, TouchedRange[]>;
}

export function createAnalysisCache(): AnalysisCache {
  return {
    commits: new Map(),
    parents: new Map(),
    blameRanges: new Map(),
    patches: new Map(),
    touchedRanges: new Map()
  };
}

export function cacheKey(parts: Array<string | number | undefined>): string {
  return parts.map((part) => String(part ?? '')).join('\u0000');
}
```

- [ ] **Step 6: 创建核心导出**

创建 `src/analysis-core/index.ts`：

```ts
export type {
  AnalyzeRequest,
  FileOverlayAnalysisRequest,
  TraceBlockRequest
} from './contracts/analysisRequest';
export type { AnalysisCliResult, TraceBlockResult } from './contracts/analysisResult';
export { createAnalysisCache, cacheKey } from './cache/analysisCache';
```

- [ ] **Step 7: 更新 Node 类型检查范围**

修改 `tsconfig.node.json` 的 `include`，加入：

```json
    "src/analysis-core/**/*.ts",
    "src/cli/**/*.ts",
```

保留原有 include 项。

- [ ] **Step 8: 运行核心契约测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/analysisCoreTypes.test.ts
```

预期：通过。

- [ ] **Step 9: 提交**

运行：

```bash
git add src/analysis-core tests/unit/analysisCoreTypes.test.ts tsconfig.node.json
git commit -m "feat: 添加分析核心契约"
```

## 任务 3：抽出可复用的文件 overlay 服务

**Files:**
- Create: `src/analysis-core/overlay/fileOverlayService.ts`
- Modify: `src/analysis-core/index.ts`
- Modify: `src/main/ipc/reviewIpc.ts`
- Test: `tests/unit/reviewIpc.test.ts`

- [ ] **Step 1: 写 overlay 服务测试**

创建或追加 `tests/unit/attributionService.test.ts`：

```ts
import { buildFileOverlay } from '../../src/analysis-core';
import type { ChangedFile } from '../../src/shared/reviewTypes';

const file: ChangedFile = {
  path: 'src/app.ts',
  status: 'modified',
  additions: 1,
  deletions: 1,
  isBinary: false,
  isPreviewable: true
};

describe('analysis-core file overlay service', () => {
  it('builds a range overlay and marks empty attribution as inferred until resolvers are attached', async () => {
    const overlay = await buildFileOverlay({
      repoPath: 'E:/repo/app',
      file,
      range: {
        branch: 'main',
        baseCommit: 'base',
        headCommit: 'head'
      },
      filters: {
        projectId: 'project-1',
        branch: 'main',
        globRules: []
      },
      rangeCommits: [],
      git: {
        readFileAtCommit: async (_repoPath, commitHash) =>
          commitHash === 'base' ? 'export const value = 1;\n' : 'export const value = 2;\n',
        showFilePatch: async () => '',
        blameFileRange: async () => [],
        listParents: async () => [],
        getCommit: async () => undefined
      }
    });

    expect(overlay.blocks).toHaveLength(1);
    expect(overlay.blocks[0].attribution?.confidence).toBe('inferred');
    expect(overlay.blocks[0].authors).toEqual([]);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/attributionService.test.ts
```

预期：失败，`buildFileOverlay` 尚未导出。

- [ ] **Step 3: 创建 overlay 服务**

创建 `src/analysis-core/overlay/fileOverlayService.ts`：

```ts
import type { GitCommitSummary } from '../../main/git/gitTypes';
import { buildFileOverlayDiff } from '../../main/analysis/overlayEngine';
import type {
  AnalysisRange,
  ChangedFile,
  FileOverlay,
  ReviewFilters
} from '../../shared/reviewTypes';
import type { AnalysisGitClient } from '../git/analysisGitClient';

export interface BuildFileOverlayCoreInput {
  repoPath: string;
  file: ChangedFile;
  range: AnalysisRange;
  rangeCommits: GitCommitSummary[];
  filters: ReviewFilters;
  git: Pick<AnalysisGitClient, 'readFileAtCommit' | 'showFilePatch' | 'blameFileRange' | 'listParents' | 'getCommit'>;
}

export async function buildFileOverlay({
  repoPath,
  file,
  range,
  git
}: BuildFileOverlayCoreInput): Promise<FileOverlay> {
  const oldText = await git.readFileAtCommit(repoPath, range.baseCommit, file.oldPath ?? file.path);
  const newText = await git.readFileAtCommit(repoPath, range.headCommit, file.path);
  const diff = buildFileOverlayDiff({ file, oldText, newText });

  return {
    mode: 'range',
    file,
    range,
    rows: diff.rows,
    blocks: diff.blocks.map((block) => ({
      ...block,
      attribution: {
        confidence: 'inferred',
        warnings: [
          {
            code: 'BLAME_UNAVAILABLE',
            message: '无法使用 blame 结果，已根据提交差异推断'
          }
        ]
      }
    })),
    warnings: []
  };
}
```

这一步先接通 overlay 核心入口，后续任务替换为真实归因服务。

- [ ] **Step 4: 创建 Git 接口最小定义**

创建 `src/analysis-core/git/analysisGitClient.ts`：

```ts
import type { ChangedFile } from '../../shared/reviewTypes';
import type { GitCommitSummary } from '../../main/git/gitTypes';

export interface BlameLine {
  commitHash: string;
  originalLine: number;
  finalLine: number;
  authorName: string;
  authorEmail?: string;
  committedAt: string;
  summary: string;
  content: string;
}

export interface AnalysisGitClient {
  listCommits(repoPath: string, branch: string): Promise<GitCommitSummary[]>;
  listChangedFiles(repoPath: string, baseCommit: string, headCommit: string): Promise<ChangedFile[]>;
  readFileAtCommit(repoPath: string, commit: string, filePath: string): Promise<string>;
  showFilePatch(repoPath: string, commit: string, filePath: string): Promise<string>;
  blameFileRange(
    repoPath: string,
    commit: string,
    filePath: string,
    startLine: number,
    endLine: number
  ): Promise<BlameLine[]>;
  listParents(repoPath: string, commitHash: string): Promise<string[]>;
  getCommit(repoPath: string, commitHash: string): Promise<GitCommitSummary | undefined>;
}
```

- [ ] **Step 5: 导出 overlay 服务**

修改 `src/analysis-core/index.ts`：

```ts
export type {
  AnalyzeRequest,
  FileOverlayAnalysisRequest,
  TraceBlockRequest
} from './contracts/analysisRequest';
export type { AnalysisCliResult, TraceBlockResult } from './contracts/analysisResult';
export { createAnalysisCache, cacheKey } from './cache/analysisCache';
export type { AnalysisGitClient, BlameLine } from './git/analysisGitClient';
export { buildFileOverlay } from './overlay/fileOverlayService';
```

- [ ] **Step 6: 运行 overlay 服务测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/attributionService.test.ts
```

预期：通过。

- [ ] **Step 7: 提交**

运行：

```bash
git add src/analysis-core tests/unit/attributionService.test.ts
git commit -m "feat: 抽出文件 overlay 分析核心"
```

## 任务 4：实现 CLI 编译和参数解析

**Files:**
- Create: `src/cli/revier-analysis.ts`
- Create: `tsconfig.cli.json`
- Modify: `package.json`
- Test: `tests/unit/analysisCli.test.ts`

- [ ] **Step 1: 写 CLI 参数解析测试**

创建 `tests/unit/analysisCli.test.ts`：

```ts
import { parseCliArgs } from '../../src/cli/revier-analysis';

describe('revier-analysis cli', () => {
  it('parses analyze command with clap-style long options', () => {
    const parsed = parseCliArgs([
      'analyze',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--glob',
      'src/**/*.ts',
      '--glob',
      '!**/*.test.ts',
      '--format',
      'json',
      '--pretty'
    ]);

    expect(parsed).toEqual({
      command: 'analyze',
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      globRules: ['src/**/*.ts', '!**/*.test.ts'],
      filePath: undefined,
      blockId: undefined,
      format: 'json',
      pretty: true,
      explain: false
    });
  });

  it('rejects missing required arguments with exit code 2 semantics', () => {
    expect(() => parseCliArgs(['file-overlay', '--repo', 'E:/repo/app'])).toThrow(
      'file-overlay 需要 --repo、--base、--head 和 --file'
    );
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/analysisCli.test.ts
```

预期：失败，CLI 文件尚不存在。

- [ ] **Step 3: 创建 CLI 入口**

创建 `src/cli/revier-analysis.ts`：

```ts
import { GitService } from '../main/git/gitService';
import { buildFileOverlay } from '../analysis-core';
import type { ChangedFile, ReviewFilters } from '../shared/reviewTypes';

export type CliCommand = 'analyze' | 'file-overlay' | 'trace-block';

export interface ParsedCliArgs {
  command: CliCommand;
  repoPath: string;
  baseCommit: string;
  headCommit: string;
  globRules: string[];
  filePath?: string;
  blockId?: string;
  format: 'json';
  pretty: boolean;
  explain: boolean;
}

export function parseCliArgs(argv: string[]): ParsedCliArgs {
  const [commandValue, ...rest] = argv;
  if (commandValue !== 'analyze' && commandValue !== 'file-overlay' && commandValue !== 'trace-block') {
    throw new Error('命令必须是 analyze、file-overlay 或 trace-block');
  }

  const values = new Map<string, string[]>();
  for (let index = 0; index < rest.length; index += 1) {
    const token = rest[index];
    if (!token.startsWith('--')) {
      throw new Error(`无法识别参数 ${token}`);
    }
    const key = token.slice(2);
    if (key === 'pretty' || key === 'explain') {
      values.set(key, ['true']);
      continue;
    }
    const value = rest[index + 1];
    if (!value || value.startsWith('--')) {
      throw new Error(`参数 --${key} 需要值`);
    }
    values.set(key, [...(values.get(key) ?? []), value]);
    index += 1;
  }

  const parsed: ParsedCliArgs = {
    command: commandValue,
    repoPath: one(values, 'repo') ?? '',
    baseCommit: one(values, 'base') ?? '',
    headCommit: one(values, 'head') ?? '',
    globRules: values.get('glob') ?? [],
    filePath: one(values, 'file'),
    blockId: one(values, 'block-id'),
    format: (one(values, 'format') ?? 'json') as 'json',
    pretty: values.has('pretty'),
    explain: values.has('explain')
  };

  if (parsed.format !== 'json') {
    throw new Error('当前仅支持 --format json');
  }
  if (!parsed.repoPath || !parsed.baseCommit || !parsed.headCommit) {
    throw new Error(`${parsed.command} 需要 --repo、--base 和 --head`);
  }
  if (parsed.command === 'file-overlay' && !parsed.filePath) {
    throw new Error('file-overlay 需要 --repo、--base、--head 和 --file');
  }
  if (parsed.command === 'trace-block' && (!parsed.filePath || !parsed.blockId)) {
    throw new Error('trace-block 需要 --repo、--base、--head、--file 和 --block-id');
  }

  return parsed;
}

function one(values: Map<string, string[]>, key: string): string | undefined {
  return values.get(key)?.at(-1);
}

async function main(): Promise<void> {
  try {
    const parsed = parseCliArgs(process.argv.slice(2));
    const git = new GitService();
    const filters: ReviewFilters = {
      projectId: 'cli',
      branch: parsed.headCommit,
      globRules: parsed.globRules
    };

    if (parsed.command === 'analyze') {
      const files = await git.listChangedFiles(parsed.repoPath, parsed.baseCommit, parsed.headCommit);
      const output = {
        version: 1,
        range: {
          baseCommit: parsed.baseCommit,
          headCommit: parsed.headCommit
        },
        files,
        overlays: [],
        warnings: []
      };
      process.stdout.write(JSON.stringify(output, null, parsed.pretty ? 2 : 0));
      process.stdout.write('\n');
      return;
    }

    const file: ChangedFile = {
      path: parsed.filePath ?? '',
      status: 'modified',
      additions: 0,
      deletions: 0,
      isBinary: false,
      isPreviewable: true
    };
    const overlay = await buildFileOverlay({
      repoPath: parsed.repoPath,
      file,
      range: {
        branch: parsed.headCommit,
        baseCommit: parsed.baseCommit,
        headCommit: parsed.headCommit
      },
      rangeCommits: [],
      filters,
      git
    });
    const output = parsed.command === 'trace-block'
      ? { version: 1, overlay, blockId: parsed.blockId }
      : overlay;
    process.stdout.write(JSON.stringify(output, null, parsed.pretty ? 2 : 0));
    process.stdout.write('\n');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${message}\n`);
    process.exitCode = message.includes('需要') || message.includes('参数') || message.includes('命令') ? 2 : 10;
  }
}

if (process.argv[1]?.endsWith('revier-analysis.js')) {
  void main();
}
```

- [ ] **Step 4: 创建 CLI 编译配置**

创建 `tsconfig.cli.json`：

```json
{
  "extends": "./tsconfig.node.json",
  "compilerOptions": {
    "composite": false,
    "declaration": false,
    "sourceMap": false,
    "noEmit": false,
    "outDir": "dist-cli",
    "rootDir": ".",
    "module": "NodeNext",
    "moduleResolution": "NodeNext"
  },
  "include": [
    "src/cli/**/*.ts",
    "src/analysis-core/**/*.ts",
    "src/main/**/*.ts",
    "src/shared/**/*.ts"
  ]
}
```

- [ ] **Step 5: 添加 package 脚本**

修改 `package.json` scripts，加入：

```json
    "build:analysis-cli": "tsc -p tsconfig.cli.json",
    "revier-analysis": "pnpm build:analysis-cli && node dist-cli/src/cli/revier-analysis.js",
```

保持 JSON 逗号合法。

- [ ] **Step 6: 运行 CLI 单元测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/analysisCli.test.ts
```

预期：通过。

- [ ] **Step 7: 运行 CLI 编译**

运行：

```bash
fnm exec --using 24 pnpm build:analysis-cli
```

预期：通过，生成 `dist-cli/src/cli/revier-analysis.js`。

- [ ] **Step 8: 提交**

运行：

```bash
git add src/cli tsconfig.cli.json package.json tests/unit/analysisCli.test.ts
git commit -m "feat: 添加分析 CLI 入口"
```

## 任务 5：实现 blame 输出解析和 Git 客户端能力

**Files:**
- Create: `src/analysis-core/git/blameParser.ts`
- Create: `src/analysis-core/git/simpleGitAnalysisClient.ts`
- Modify: `src/analysis-core/git/analysisGitClient.ts`
- Modify: `src/main/git/gitService.ts`
- Test: `tests/unit/blameParser.test.ts`
- Test: `tests/integration/gitService.test.ts`

- [ ] **Step 1: 写 blame 解析测试**

创建 `tests/unit/blameParser.test.ts`：

```ts
import { parseBlamePorcelain } from '../../src/analysis-core/git/blameParser';

describe('blameParser', () => {
  it('parses line porcelain blame output', () => {
    const lines = parseBlamePorcelain(`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3 10 1
author Alice
author-mail <alice@example.com>
author-time 1780272000
summary feat: add value
\tconst value = 1;
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 4 11 1
author Bob
author-mail <bob@example.com>
author-time 1780358400
summary fix: update value
\tconst value = 2;
`);

    expect(lines).toEqual([
      {
        commitHash: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        originalLine: 3,
        finalLine: 10,
        authorName: 'Alice',
        authorEmail: 'alice@example.com',
        committedAt: '2026-06-01T00:00:00.000Z',
        summary: 'feat: add value',
        content: 'const value = 1;'
      },
      {
        commitHash: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
        originalLine: 4,
        finalLine: 11,
        authorName: 'Bob',
        authorEmail: 'bob@example.com',
        committedAt: '2026-06-02T00:00:00.000Z',
        summary: 'fix: update value',
        content: 'const value = 2;'
      }
    ]);
  });
});
```

- [ ] **Step 2: 运行解析测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/blameParser.test.ts
```

预期：失败，解析器不存在。

- [ ] **Step 3: 实现 blame 解析器**

创建 `src/analysis-core/git/blameParser.ts`：

```ts
import type { BlameLine } from './analysisGitClient';

export function parseBlamePorcelain(output: string): BlameLine[] {
  const result: BlameLine[] = [];
  let current:
    | {
        commitHash: string;
        originalLine: number;
        finalLine: number;
        authorName?: string;
        authorEmail?: string;
        authorTime?: number;
        summary?: string;
      }
    | undefined;

  for (const rawLine of output.split(/\r?\n/)) {
    const header = rawLine.match(/^([0-9a-f]{40}) (\d+) (\d+)(?: \d+)?$/);
    if (header) {
      current = {
        commitHash: header[1],
        originalLine: Number(header[2]),
        finalLine: Number(header[3])
      };
      continue;
    }

    if (!current) {
      continue;
    }

    if (rawLine.startsWith('author ')) {
      current.authorName = rawLine.slice('author '.length);
      continue;
    }
    if (rawLine.startsWith('author-mail ')) {
      const email = rawLine.slice('author-mail '.length).trim();
      current.authorEmail = email.replace(/^<|>$/g, '') || undefined;
      continue;
    }
    if (rawLine.startsWith('author-time ')) {
      current.authorTime = Number(rawLine.slice('author-time '.length));
      continue;
    }
    if (rawLine.startsWith('summary ')) {
      current.summary = rawLine.slice('summary '.length);
      continue;
    }
    if (rawLine.startsWith('\t')) {
      result.push({
        commitHash: current.commitHash,
        originalLine: current.originalLine,
        finalLine: current.finalLine,
        authorName: current.authorName ?? '未知作者',
        authorEmail: current.authorEmail,
        committedAt: new Date((current.authorTime ?? 0) * 1000).toISOString(),
        summary: current.summary ?? '',
        content: rawLine.slice(1)
      });
      current = undefined;
    }
  }

  return result;
}
```

- [ ] **Step 4: 实现 SimpleGitAnalysisClient**

创建 `src/analysis-core/git/simpleGitAnalysisClient.ts`：

```ts
import simpleGit from 'simple-git';
import { normalizeGitPath } from '../../main/analysis/changedFiles';
import type { ChangedFile } from '../../shared/reviewTypes';
import type { GitCommitSummary } from '../../main/git/gitTypes';
import { GitService } from '../../main/git/gitService';
import type { AnalysisGitClient, BlameLine } from './analysisGitClient';
import { parseBlamePorcelain } from './blameParser';

export class SimpleGitAnalysisClient implements AnalysisGitClient {
  private readonly gitService = new GitService();

  listCommits(repoPath: string, branch: string): Promise<GitCommitSummary[]> {
    return this.gitService.listCommits(repoPath, branch);
  }

  listChangedFiles(repoPath: string, baseCommit: string, headCommit: string): Promise<ChangedFile[]> {
    return this.gitService.listChangedFiles(repoPath, baseCommit, headCommit);
  }

  readFileAtCommit(repoPath: string, commit: string, filePath: string): Promise<string> {
    return this.gitService.readFileAtCommit(repoPath, commit, filePath);
  }

  showFilePatch(repoPath: string, commit: string, filePath: string): Promise<string> {
    return this.gitService.showFilePatch(repoPath, commit, filePath);
  }

  async blameFileRange(
    repoPath: string,
    commit: string,
    filePath: string,
    startLine: number,
    endLine: number
  ): Promise<BlameLine[]> {
    if (startLine <= 0 || endLine < startLine) {
      return [];
    }
    const output = await simpleGit(repoPath).raw([
      'blame',
      '--line-porcelain',
      '-M',
      '-C',
      `-L${startLine},${endLine}`,
      commit,
      '--',
      normalizeGitPath(filePath)
    ]);
    return parseBlamePorcelain(output);
  }

  async listParents(repoPath: string, commitHash: string): Promise<string[]> {
    const line = (await simpleGit(repoPath).raw(['rev-list', '--parents', '-n', '1', commitHash])).trim();
    const [, ...parents] = line.split(/\s+/);
    return parents;
  }

  async getCommit(repoPath: string, commitHash: string): Promise<GitCommitSummary | undefined> {
    const output = await simpleGit(repoPath).raw([
      'show',
      '--no-patch',
      '--date=iso-strict',
      '--format=%H%x00%an%x00%ae%x00%aI%x00%s',
      commitHash
    ]);
    const [hash, authorName, authorEmail, committedAt, subject] = output.trim().split('\u0000');
    if (!hash) {
      return undefined;
    }
    return {
      hash,
      shortHash: hash.slice(0, 8),
      authorName,
      authorEmail,
      committedAt: new Date(committedAt).toISOString(),
      subject
    };
  }
}
```

- [ ] **Step 5: 让 GitService 实现新接口能力**

修改 `src/main/git/gitService.ts`，导入：

```ts
import { SimpleGitAnalysisClient } from '../../analysis-core/git/simpleGitAnalysisClient';
import type { BlameLine } from '../../analysis-core/git/analysisGitClient';
```

在 `GitService` 类内增加：

```ts
  private readonly analysisClient = new SimpleGitAnalysisClient();

  async blameFileRange(
    repoPath: string,
    commit: string,
    filePath: string,
    startLine: number,
    endLine: number
  ): Promise<BlameLine[]> {
    return this.analysisClient.blameFileRange(repoPath, commit, filePath, startLine, endLine);
  }

  async listParents(repoPath: string, commitHash: string): Promise<string[]> {
    return this.analysisClient.listParents(repoPath, commitHash);
  }

  async getCommit(repoPath: string, commitHash: string): Promise<GitCommitSummary | undefined> {
    return this.analysisClient.getCommit(repoPath, commitHash);
  }
```

如果循环依赖导致运行时报错，则改为把 `GitService` 内现有通用方法下沉到 `SimpleGitAnalysisClient`，再让 `GitService` 继承或组合该客户端。以测试结果为准。

- [ ] **Step 6: 写 Git 集成测试**

追加到 `tests/integration/gitService.test.ts`：

```ts
  it('blames a file range and lists all parents', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-git-blame-'));

    try {
      const repo = await initTestRepo(dir);
      const first = await commitFiles(repo, {
        message: 'feat: alice line',
        authorName: 'Alice',
        authorEmail: 'alice@example.com',
        date: '2026-06-01T00:00:00.000Z',
        files: { 'src/app.ts': 'export const value = 1;\n' }
      });
      const second = await commitFiles(repo, {
        message: 'fix: bob line',
        authorName: 'Bob',
        authorEmail: 'bob@example.com',
        date: '2026-06-02T00:00:00.000Z',
        files: { 'src/app.ts': 'export const value = 2;\n' }
      });

      const service = new GitService();
      const blame = await service.blameFileRange(dir, second, 'src/app.ts', 1, 1);
      const parents = await service.listParents(dir, second);
      const commit = await service.getCommit(dir, second);

      expect(blame[0].commitHash).toBe(second);
      expect(blame[0].authorName).toBe('Bob');
      expect(parents).toEqual([first]);
      expect(commit?.subject).toBe('fix: bob line');
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
```

- [ ] **Step 7: 运行 Git 能力测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/blameParser.test.ts tests/integration/gitService.test.ts
```

预期：通过。

- [ ] **Step 8: 提交**

运行：

```bash
git add src/analysis-core/git src/main/git/gitService.ts tests/unit/blameParser.test.ts tests/integration/gitService.test.ts
git commit -m "feat: 添加 blame Git 分析能力"
```

## 任务 6：实现 patch 推断归因

**Files:**
- Create: `src/analysis-core/attribution/patchInferenceResolver.ts`
- Modify: `src/analysis-core/index.ts`
- Test: `tests/unit/attributionService.test.ts`

- [ ] **Step 1: 写 patch 推断测试**

追加到 `tests/unit/attributionService.test.ts`：

```ts
import { inferAttributionFromPatches } from '../../src/analysis-core';
import type { DiffBlock, RelatedCommit } from '../../src/shared/reviewTypes';

describe('patch inference attribution', () => {
  it('marks matching patch attribution as inferred', () => {
    const block: DiffBlock = {
      id: 'block-1',
      oldStart: 1,
      oldEnd: 1,
      newStart: 1,
      newEnd: 1,
      changeType: 'modified',
      authors: [],
      rows: [],
      relatedCommits: []
    };
    const commit: RelatedCommit = {
      hash: 'a'.repeat(40),
      shortHash: 'aaaaaaaa',
      authorName: 'Alice',
      authorEmail: 'alice@example.com',
      committedAt: '2026-06-01T00:00:00.000Z',
      subject: 'feat: patch',
      matchedByFilter: true,
      touchedRanges: [{ oldStart: 1, oldEnd: 1, newStart: 1, newEnd: 1 }]
    };

    const [attributed] = inferAttributionFromPatches([block], [commit]);

    expect(attributed.authors).toEqual([{ name: 'Alice', email: 'alice@example.com' }]);
    expect(attributed.relatedCommits[0].attribution?.method).toBe('patch-inference');
    expect(attributed.attribution?.confidence).toBe('inferred');
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/attributionService.test.ts
```

预期：失败，`inferAttributionFromPatches` 尚不存在。

- [ ] **Step 3: 实现 patch 推断**

创建 `src/analysis-core/attribution/patchInferenceResolver.ts`：

```ts
import type { AuthorSummary, DiffBlock, RelatedCommit, TouchedRange } from '../../shared/reviewTypes';
import { rangesIntersect } from '../../main/analysis/patchRanges';

export function inferAttributionFromPatches(
  blocks: DiffBlock[],
  commits: RelatedCommit[]
): DiffBlock[] {
  return blocks.map((block) => {
    const blockRange: TouchedRange = {
      oldStart: block.oldStart || undefined,
      oldEnd: block.oldEnd || undefined,
      newStart: block.newStart || undefined,
      newEnd: block.newEnd || undefined
    };
    const relatedCommits = commits
      .filter((commit) => commit.touchedRanges.some((range) => rangesIntersect(blockRange, range)))
      .map((commit) => ({
        ...commit,
        attribution: {
          method: 'patch-inference' as const,
          viaMergeHashes: commit.attribution?.viaMergeHashes ?? []
        }
      }));

    return {
      ...block,
      authors: uniqueAuthors(relatedCommits),
      relatedCommits,
      attribution: {
        confidence: 'inferred',
        warnings: [
          {
            code: 'BLAME_UNAVAILABLE',
            message: '无法使用 blame 结果，已根据提交差异推断'
          }
        ]
      }
    };
  });
}

function uniqueAuthors(commits: RelatedCommit[]): AuthorSummary[] {
  const authors = new Map<string, AuthorSummary>();
  for (const commit of commits) {
    const key = `${commit.authorName}<${commit.authorEmail ?? ''}>`;
    authors.set(key, { name: commit.authorName, email: commit.authorEmail });
  }
  return [...authors.values()];
}
```

- [ ] **Step 4: 导出 patch 推断**

修改 `src/analysis-core/index.ts`，追加：

```ts
export { inferAttributionFromPatches } from './attribution/patchInferenceResolver';
```

- [ ] **Step 5: 运行测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/attributionService.test.ts
```

预期：通过。

- [ ] **Step 6: 提交**

运行：

```bash
git add src/analysis-core/attribution src/analysis-core/index.ts tests/unit/attributionService.test.ts
git commit -m "feat: 添加提交差异推断归因"
```

## 任务 7：实现 blame 主归因

**Files:**
- Create: `src/analysis-core/attribution/blameOriginResolver.ts`
- Create: `src/analysis-core/attribution/attributionService.ts`
- Modify: `src/analysis-core/overlay/fileOverlayService.ts`
- Modify: `src/analysis-core/index.ts`
- Test: `tests/unit/attributionService.test.ts`

- [ ] **Step 1: 写 blame 精确归因测试**

追加到 `tests/unit/attributionService.test.ts`：

```ts
import { attributeBlocks } from '../../src/analysis-core';

describe('blame attribution', () => {
  it('attributes added and modified new-side lines to blame authors', async () => {
    const attributed = await attributeBlocks({
      repoPath: 'E:/repo/app',
      headCommit: 'head',
      filePath: 'src/app.ts',
      blocks: [
        {
          id: 'block-1',
          oldStart: 0,
          oldEnd: 0,
          newStart: 2,
          newEnd: 2,
          changeType: 'added',
          authors: [],
          rows: [],
          relatedCommits: []
        }
      ],
      relatedCommits: [],
      git: {
        blameFileRange: async () => [
          {
            commitHash: 'a'.repeat(40),
            originalLine: 2,
            finalLine: 2,
            authorName: 'Alice',
            authorEmail: 'alice@example.com',
            committedAt: '2026-06-01T00:00:00.000Z',
            summary: 'feat: line',
            content: 'const value = 1;'
          }
        ],
        listParents: async () => ['p'.repeat(40)],
        getCommit: async () => ({
          hash: 'a'.repeat(40),
          shortHash: 'aaaaaaaa',
          authorName: 'Alice',
          authorEmail: 'alice@example.com',
          committedAt: '2026-06-01T00:00:00.000Z',
          subject: 'feat: line'
        }),
        showFilePatch: async () => '',
        readFileAtCommit: async () => ''
      }
    });

    expect(attributed[0].authors).toEqual([{ name: 'Alice', email: 'alice@example.com' }]);
    expect(attributed[0].attribution?.confidence).toBe('precise');
    expect(attributed[0].relatedCommits[0].attribution?.method).toBe('blame');
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/attributionService.test.ts
```

预期：失败，`attributeBlocks` 尚不存在。

- [ ] **Step 3: 实现 blame resolver**

创建 `src/analysis-core/attribution/blameOriginResolver.ts`：

```ts
import type { RelatedCommit } from '../../shared/reviewTypes';
import type { AnalysisGitClient } from '../git/analysisGitClient';

export interface ResolveBlameOriginsInput {
  repoPath: string;
  headCommit: string;
  filePath: string;
  startLine: number;
  endLine: number;
  git: Pick<AnalysisGitClient, 'blameFileRange' | 'getCommit' | 'listParents'>;
}

export async function resolveBlameOrigins({
  repoPath,
  headCommit,
  filePath,
  startLine,
  endLine,
  git
}: ResolveBlameOriginsInput): Promise<RelatedCommit[]> {
  const blameLines = await git.blameFileRange(repoPath, headCommit, filePath, startLine, endLine);
  const commits = new Map<string, RelatedCommit>();

  for (const line of blameLines) {
    const commit = await git.getCommit(repoPath, line.commitHash);
    if (!commit) {
      continue;
    }
    commits.set(commit.hash, {
      hash: commit.hash,
      shortHash: commit.shortHash,
      authorName: commit.authorName,
      authorEmail: commit.authorEmail,
      committedAt: commit.committedAt,
      subject: commit.subject,
      matchedByFilter: false,
      touchedRanges: [{ newStart: line.finalLine, newEnd: line.finalLine }],
      attribution: {
        method: 'blame',
        viaMergeHashes: []
      }
    });
  }

  return [...commits.values()];
}
```

- [ ] **Step 4: 实现归因服务聚合**

创建 `src/analysis-core/attribution/attributionService.ts`：

```ts
import type { AuthorSummary, DiffBlock, RelatedCommit } from '../../shared/reviewTypes';
import type { AnalysisGitClient } from '../git/analysisGitClient';
import { inferAttributionFromPatches } from './patchInferenceResolver';
import { resolveBlameOrigins } from './blameOriginResolver';

export interface AttributeBlocksInput {
  repoPath: string;
  headCommit: string;
  filePath: string;
  blocks: DiffBlock[];
  relatedCommits: RelatedCommit[];
  git: Pick<AnalysisGitClient, 'blameFileRange' | 'getCommit' | 'listParents' | 'showFilePatch' | 'readFileAtCommit'>;
}

export async function attributeBlocks(input: AttributeBlocksInput): Promise<DiffBlock[]> {
  const attributed: DiffBlock[] = [];

  for (const block of input.blocks) {
    if (block.newStart > 0 && block.newEnd >= block.newStart) {
      const blameCommits = await resolveBlameOrigins({
        repoPath: input.repoPath,
        headCommit: input.headCommit,
        filePath: input.filePath,
        startLine: block.newStart,
        endLine: block.newEnd,
        git: input.git
      });

      if (blameCommits.length > 0) {
        attributed.push({
          ...block,
          authors: uniqueAuthors(blameCommits),
          relatedCommits: blameCommits,
          attribution: {
            confidence: 'precise',
            warnings: []
          }
        });
        continue;
      }
    }

    attributed.push(inferAttributionFromPatches([block], input.relatedCommits)[0]);
  }

  return attributed;
}

function uniqueAuthors(commits: RelatedCommit[]): AuthorSummary[] {
  const authors = new Map<string, AuthorSummary>();
  for (const commit of commits) {
    const key = `${commit.authorName}<${commit.authorEmail ?? ''}>`;
    authors.set(key, { name: commit.authorName, email: commit.authorEmail });
  }
  return [...authors.values()];
}
```

- [ ] **Step 5: 接入 overlay 服务**

修改 `src/analysis-core/overlay/fileOverlayService.ts`：

```ts
import type { GitCommitSummary } from '../../main/git/gitTypes';
import { buildFileOverlayDiff } from '../../main/analysis/overlayEngine';
import { parsePatchTouchedRanges } from '../../main/analysis/patchRanges';
import type {
  AnalysisRange,
  ChangedFile,
  FileOverlay,
  RelatedCommit,
  ReviewFilters
} from '../../shared/reviewTypes';
import type { AnalysisGitClient } from '../git/analysisGitClient';
import { attributeBlocks } from '../attribution/attributionService';

export interface BuildFileOverlayCoreInput {
  repoPath: string;
  file: ChangedFile;
  range: AnalysisRange;
  rangeCommits: GitCommitSummary[];
  filters: ReviewFilters;
  git: Pick<
    AnalysisGitClient,
    'readFileAtCommit' | 'showFilePatch' | 'blameFileRange' | 'listParents' | 'getCommit'
  >;
}

export async function buildFileOverlay({
  repoPath,
  file,
  range,
  rangeCommits,
  filters,
  git
}: BuildFileOverlayCoreInput): Promise<FileOverlay> {
  const oldText = await git.readFileAtCommit(repoPath, range.baseCommit, file.oldPath ?? file.path);
  const newText = await git.readFileAtCommit(repoPath, range.headCommit, file.path);
  const diff = buildFileOverlayDiff({ file, oldText, newText });
  const relatedCommits = await buildRelatedCommits(repoPath, file, rangeCommits, filters, git);
  const blocks = await attributeBlocks({
    repoPath,
    headCommit: range.headCommit,
    filePath: file.path,
    blocks: diff.blocks,
    relatedCommits,
    git
  });

  return {
    mode: 'range',
    file,
    range,
    rows: diff.rows,
    blocks,
    warnings: []
  };
}

async function buildRelatedCommits(
  repoPath: string,
  file: ChangedFile,
  rangeCommits: GitCommitSummary[],
  filters: ReviewFilters,
  git: Pick<AnalysisGitClient, 'showFilePatch'>
): Promise<RelatedCommit[]> {
  const markMatches = hasDisplayCommitFilters(filters);
  const commits: RelatedCommit[] = [];
  const paths = [...new Set([file.path, file.oldPath].filter((path): path is string => Boolean(path)))];

  for (const commit of rangeCommits) {
    const touchedRanges = [];
    for (const path of paths) {
      const patch = await git.showFilePatch(repoPath, commit.hash, path);
      touchedRanges.push(...parsePatchTouchedRanges(patch));
    }
    if (touchedRanges.length === 0) {
      continue;
    }
    commits.push({
      hash: commit.hash,
      shortHash: commit.shortHash,
      authorName: commit.authorName,
      authorEmail: commit.authorEmail,
      committedAt: commit.committedAt,
      subject: commit.subject,
      matchedByFilter: markMatches ? commitMatchesDisplayFilters(commit, filters) : false,
      touchedRanges
    });
  }

  return commits;
}

function hasDisplayCommitFilters(filters: ReviewFilters): boolean {
  return Boolean((filters.authorKeys?.length ?? 0) > 0 || filters.authorQuery?.trim() || filters.messageQuery?.trim());
}

function commitMatchesDisplayFilters(commit: GitCommitSummary, filters: ReviewFilters): boolean {
  return (
    matchesAuthorKeys(commit, filters.authorKeys ?? []) &&
    matchesQuery(`${commit.authorName} ${commit.authorEmail ?? ''}`, filters.authorQuery) &&
    matchesQuery(commit.subject, filters.messageQuery)
  );
}

function matchesAuthorKeys(commit: GitCommitSummary, authorKeys: string[]): boolean {
  if (authorKeys.length === 0) {
    return true;
  }
  const normalizedKeys = new Set(authorKeys.map((key) => key.toLowerCase()));
  return normalizedKeys.has(authorKey(commit.authorName, commit.authorEmail));
}

function matchesQuery(value: string, query?: string): boolean {
  const normalized = query?.trim().toLowerCase();
  return !normalized || value.toLowerCase().includes(normalized);
}

function authorKey(name: string, email?: string): string {
  return (email?.trim() || name.trim()).toLowerCase();
}
```

- [ ] **Step 6: 导出归因服务**

修改 `src/analysis-core/index.ts`，追加：

```ts
export { attributeBlocks } from './attribution/attributionService';
export { resolveBlameOrigins } from './attribution/blameOriginResolver';
```

- [ ] **Step 7: 运行归因服务测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/attributionService.test.ts
```

预期：通过。

- [ ] **Step 8: 提交**

运行：

```bash
git add src/analysis-core tests/unit/attributionService.test.ts
git commit -m "feat: 添加 blame 主归因服务"
```

## 任务 8：实现 merge 父链追踪

**Files:**
- Create: `src/analysis-core/attribution/mergeOriginResolver.ts`
- Modify: `src/analysis-core/attribution/blameOriginResolver.ts`
- Modify: `src/analysis-core/attribution/attributionService.ts`
- Modify: `src/analysis-core/index.ts`
- Test: `tests/unit/attributionService.test.ts`

- [ ] **Step 1: 写 merge 链路单元测试**

追加到 `tests/unit/attributionService.test.ts`：

```ts
import { resolveMergeOrigin } from '../../src/analysis-core';

describe('merge origin resolver', () => {
  it('traces a merge commit blame result to the parent original commit containing the same content', async () => {
    const original = 'o'.repeat(40);
    const merge = 'm'.repeat(40);
    const left = 'l'.repeat(40);
    const right = 'r'.repeat(40);

    const origin = await resolveMergeOrigin({
      repoPath: 'E:/repo/app',
      mergeCommitHash: merge,
      filePath: 'src/app.ts',
      content: 'export const value = 2;',
      git: {
        listParents: async (repoPath, commitHash) => {
          expect(repoPath).toBe('E:/repo/app');
          return commitHash === merge ? [left, right] : [];
        },
        readFileAtCommit: async (_repoPath, commitHash) =>
          commitHash === right ? 'export const value = 2;\n' : 'export const value = 1;\n',
        getCommit: async (_repoPath, commitHash) => ({
          hash: commitHash === right ? original : commitHash,
          shortHash: commitHash.slice(0, 8),
          authorName: commitHash === right ? 'Alice' : 'Merge Bot',
          authorEmail: commitHash === right ? 'alice@example.com' : 'merge@example.com',
          committedAt: '2026-06-01T00:00:00.000Z',
          subject: commitHash === right ? 'feat: original' : 'merge branch'
        })
      }
    });

    expect(origin.commits[0].authorName).toBe('Alice');
    expect(origin.commits[0].attribution?.method).toBe('merge-trace');
    expect(origin.commits[0].attribution?.viaMergeHashes).toEqual([merge]);
    expect(origin.ambiguous).toBe(false);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/attributionService.test.ts
```

预期：失败，`resolveMergeOrigin` 尚不存在。

- [ ] **Step 3: 实现 merge 追踪**

创建 `src/analysis-core/attribution/mergeOriginResolver.ts`：

```ts
import type { RelatedCommit } from '../../shared/reviewTypes';
import type { AnalysisGitClient } from '../git/analysisGitClient';

export interface ResolveMergeOriginInput {
  repoPath: string;
  mergeCommitHash: string;
  filePath: string;
  content: string;
  git: Pick<AnalysisGitClient, 'listParents' | 'readFileAtCommit' | 'getCommit'>;
  visited?: Set<string>;
  viaMergeHashes?: string[];
}

export interface ResolveMergeOriginResult {
  commits: RelatedCommit[];
  ambiguous: boolean;
}

export async function resolveMergeOrigin(input: ResolveMergeOriginInput): Promise<ResolveMergeOriginResult> {
  const visited = input.visited ?? new Set<string>();
  const visitKey = `${input.mergeCommitHash}:${input.filePath}:${input.content}`;
  if (visited.has(visitKey)) {
    return { commits: [], ambiguous: true };
  }
  visited.add(visitKey);

  const parents = await input.git.listParents(input.repoPath, input.mergeCommitHash);
  const matchingParents: string[] = [];
  for (const parent of parents) {
    const parentText = await input.git.readFileAtCommit(input.repoPath, parent, input.filePath);
    if (parentText.includes(input.content)) {
      matchingParents.push(parent);
    }
  }

  if (matchingParents.length === 0) {
    const mergeCommit = await input.git.getCommit(input.repoPath, input.mergeCommitHash);
    return {
      commits: mergeCommit
        ? [
            {
              hash: mergeCommit.hash,
              shortHash: mergeCommit.shortHash,
              authorName: mergeCommit.authorName,
              authorEmail: mergeCommit.authorEmail,
              committedAt: mergeCommit.committedAt,
              subject: mergeCommit.subject,
              matchedByFilter: false,
              touchedRanges: [],
              attribution: {
                method: 'merge-trace',
                viaMergeHashes: input.viaMergeHashes ?? [input.mergeCommitHash]
              }
            }
          ]
        : [],
      ambiguous: false
    };
  }

  const commits: RelatedCommit[] = [];
  for (const parent of matchingParents) {
    const parentParents = await input.git.listParents(input.repoPath, parent);
    if (parentParents.length > 1) {
      const nested = await resolveMergeOrigin({
        ...input,
        mergeCommitHash: parent,
        visited,
        viaMergeHashes: [...(input.viaMergeHashes ?? [input.mergeCommitHash]), parent]
      });
      commits.push(...nested.commits);
      continue;
    }
    const commit = await input.git.getCommit(input.repoPath, parent);
    if (commit) {
      commits.push({
        hash: commit.hash,
        shortHash: commit.shortHash,
        authorName: commit.authorName,
        authorEmail: commit.authorEmail,
        committedAt: commit.committedAt,
        subject: commit.subject,
        matchedByFilter: false,
        touchedRanges: [],
        attribution: {
          method: 'merge-trace',
          viaMergeHashes: input.viaMergeHashes ?? [input.mergeCommitHash]
        }
      });
    }
  }

  return {
    commits: uniqueByHash(commits),
    ambiguous: matchingParents.length > 1
  };
}

function uniqueByHash(commits: RelatedCommit[]): RelatedCommit[] {
  return [...new Map(commits.map((commit) => [commit.hash, commit])).values()];
}
```

- [ ] **Step 4: blame resolver 调用 merge 追踪**

修改 `src/analysis-core/attribution/blameOriginResolver.ts`，在拿到 `commit` 后判断父提交数量：

```ts
import { resolveMergeOrigin } from './mergeOriginResolver';
```

把 `commits.set(commit.hash, ...)` 所在逻辑替换为：

```ts
    const parents = await git.listParents(repoPath, commit.hash);
    if (parents.length > 1) {
      const traced = await resolveMergeOrigin({
        repoPath,
        mergeCommitHash: commit.hash,
        filePath,
        content: line.content,
        git
      });
      for (const tracedCommit of traced.commits) {
        commits.set(tracedCommit.hash, {
          ...tracedCommit,
          touchedRanges: [{ newStart: line.finalLine, newEnd: line.finalLine }]
        });
      }
      continue;
    }

    commits.set(commit.hash, {
      hash: commit.hash,
      shortHash: commit.shortHash,
      authorName: commit.authorName,
      authorEmail: commit.authorEmail,
      committedAt: commit.committedAt,
      subject: commit.subject,
      matchedByFilter: false,
      touchedRanges: [{ newStart: line.finalLine, newEnd: line.finalLine }],
      attribution: {
        method: 'blame',
        viaMergeHashes: []
      }
    });
```

- [ ] **Step 5: 局部歧义映射到 partial**

修改 `attributeBlocks` 中 blame 成功分支：

```ts
      const hasAmbiguousMerge = blameCommits.some(
        (commit) => (commit.attribution?.viaMergeHashes.length ?? 0) > 0 && commit.touchedRanges.length === 0
      );
      attributed.push({
        ...block,
        authors: uniqueAuthors(blameCommits),
        relatedCommits: blameCommits,
        attribution: {
          confidence: hasAmbiguousMerge ? 'partial' : 'precise',
          warnings: hasAmbiguousMerge
            ? [
                {
                  code: 'MERGE_TRACE_AMBIGUOUS',
                  message: '部分行存在多条可能的 merge 来源'
                }
              ]
            : []
        }
      });
```

如果测试发现 `touchedRanges.length === 0` 不能代表歧义，则在 `resolveBlameOrigins` 返回结构中显式增加 `ambiguous`，再由 `attributeBlocks` 设置 `partial`。

- [ ] **Step 6: 导出 merge 追踪**

修改 `src/analysis-core/index.ts`，追加：

```ts
export { resolveMergeOrigin } from './attribution/mergeOriginResolver';
```

- [ ] **Step 7: 运行 merge 追踪测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/attributionService.test.ts
```

预期：通过。

- [ ] **Step 8: 提交**

运行：

```bash
git add src/analysis-core/attribution src/analysis-core/index.ts tests/unit/attributionService.test.ts
git commit -m "feat: 追踪 merge 原始提交作者"
```

## 任务 9：增加复杂 merge Git fixture 和集成测试

**Files:**
- Modify: `tests/fixtures/gitRepo.ts`
- Create: `tests/integration/mergeAttribution.test.ts`

- [ ] **Step 1: 扩展 Git fixture 工具**

修改 `tests/fixtures/gitRepo.ts`，增加导入：

```ts
import { rm } from 'node:fs/promises';
```

在文件末尾增加：

```ts
export async function checkoutBranch(repo: TestRepo, branch: string): Promise<void> {
  await repo.git.checkout(branch);
}

export async function createBranch(repo: TestRepo, branch: string): Promise<void> {
  await repo.git.checkoutLocalBranch(branch);
}

export async function checkoutNewBranch(repo: TestRepo, branch: string, startPoint: string): Promise<void> {
  await repo.git.checkout(['-b', branch, startPoint]);
}

export async function mergeBranch(repo: TestRepo, branch: string, message: string): Promise<string> {
  await repo.git.merge(['--no-ff', branch, '-m', message]);
  return (await repo.git.revparse(['HEAD'])).trim();
}

export async function removeFile(repo: TestRepo, relativePath: string, message: string): Promise<string> {
  await rm(join(repo.path, relativePath), { force: true });
  await repo.git.add('.');
  await repo.git.commit(message);
  return (await repo.git.revparse(['HEAD'])).trim();
}
```

- [ ] **Step 2: 写 merge 归因集成测试**

创建 `tests/integration/mergeAttribution.test.ts`：

```ts
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildFileOverlay } from '../../src/analysis-core';
import { GitService } from '../../src/main/git/gitService';
import type { ChangedFile, ReviewFilters } from '../../src/shared/reviewTypes';
import {
  checkoutBranch,
  checkoutNewBranch,
  commitFiles,
  initTestRepo,
  mergeBranch
} from '../fixtures/gitRepo';

describe('merge attribution integration', () => {
  it('attributes code introduced before the selected range to the original author when merged in range', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-merge-attribution-'));

    try {
      const repo = await initTestRepo(dir);
      const base = await commitFiles(repo, {
        message: 'base app',
        date: '2026-05-01T00:00:00.000Z',
        files: { 'src/app.ts': 'export const value = 1;\n' }
      });

      await checkoutNewBranch(repo, 'feature/alice', base);
      const original = await commitFiles(repo, {
        message: 'feat: alice value',
        authorName: 'Alice',
        authorEmail: 'alice@example.com',
        date: '2026-05-10T00:00:00.000Z',
        files: { 'src/app.ts': 'export const value = 2;\n' }
      });

      await checkoutBranch(repo, 'master');
      await commitFiles(repo, {
        message: 'chore: main keeps moving',
        authorName: 'Main',
        authorEmail: 'main@example.com',
        date: '2026-06-01T00:00:00.000Z',
        files: { 'src/other.ts': 'export const other = true;\n' }
      });
      const merge = await mergeBranch(repo, 'feature/alice', 'merge feature alice');

      const git = new GitService();
      const file: ChangedFile = {
        path: 'src/app.ts',
        status: 'modified',
        additions: 1,
        deletions: 1,
        isBinary: false,
        isPreviewable: true
      };
      const filters: ReviewFilters = {
        projectId: 'project-1',
        branch: 'HEAD',
        startAt: '2026-06-01T00:00:00.000Z',
        endAt: '2026-06-30T00:00:00.000Z',
        globRules: []
      };

      const overlay = await buildFileOverlay({
        repoPath: dir,
        file,
        range: {
          branch: 'HEAD',
          baseCommit: base,
          headCommit: merge,
          startAt: filters.startAt,
          endAt: filters.endAt
        },
        rangeCommits: [
          {
            hash: merge,
            shortHash: merge.slice(0, 8),
            authorName: 'Test User',
            authorEmail: 'test@example.com',
            committedAt: '2026-06-01T00:00:00.000Z',
            subject: 'merge feature alice'
          },
          {
            hash: original,
            shortHash: original.slice(0, 8),
            authorName: 'Alice',
            authorEmail: 'alice@example.com',
            committedAt: '2026-05-10T00:00:00.000Z',
            subject: 'feat: alice value'
          }
        ],
        filters,
        git
      });

      expect(overlay.blocks[0].authors).toEqual([{ name: 'Alice', email: 'alice@example.com' }]);
      expect(overlay.blocks[0].relatedCommits[0].hash).toBe(original);
      expect(overlay.blocks[0].attribution?.confidence).toBe('precise');
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
```

- [ ] **Step 3: 运行集成测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/integration/mergeAttribution.test.ts
```

预期：如果当前 merge 追踪只能定位 merge 父提交而不能定位原始提交，测试会失败；修正 `mergeOriginResolver` 继续沿单父链回溯，直到内容首次出现或父提交不再包含内容。

- [ ] **Step 4: 修正 merge 回溯至原始提交**

在 `mergeOriginResolver.ts` 中，当匹配父提交不是 merge commit 时，不直接返回该父提交；继续读取该父提交的第一个父提交，如果第一个父提交不包含目标内容，则当前父提交就是原始提交。实现辅助函数：

```ts
async function traceFirstIntroducingCommit(
  repoPath: string,
  commitHash: string,
  filePath: string,
  content: string,
  git: Pick<AnalysisGitClient, 'listParents' | 'readFileAtCommit' | 'getCommit'>
): Promise<string> {
  let current = commitHash;
  for (;;) {
    const parents = await git.listParents(repoPath, current);
    const firstParent = parents[0];
    if (!firstParent) {
      return current;
    }
    const parentText = await git.readFileAtCommit(repoPath, firstParent, filePath);
    if (!parentText.includes(content)) {
      return current;
    }
    current = firstParent;
  }
}
```

在匹配父提交分支中使用该函数得到 `originHash`，再 `getCommit(originHash)`。

- [ ] **Step 5: 重新运行集成测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/integration/mergeAttribution.test.ts
```

预期：通过。

- [ ] **Step 6: 提交**

运行：

```bash
git add tests/fixtures/gitRepo.ts tests/integration/mergeAttribution.test.ts src/analysis-core/attribution/mergeOriginResolver.ts
git commit -m "test: 覆盖复杂 merge 原始作者归因"
```

## 任务 10：迁移 IPC 使用 analysis-core

**Files:**
- Modify: `src/main/ipc/reviewIpc.ts`
- Modify: `tests/unit/reviewIpc.test.ts`

- [ ] **Step 1: 更新 reviewIpc 测试 mock Git 能力**

在 `tests/unit/reviewIpc.test.ts` 的所有 mock git 对象中增加：

```ts
        blameFileRange: vi.fn(async () => []),
        listParents: vi.fn(async () => []),
        getCommit: vi.fn(async () => undefined)
```

在 “builds file overlay from real base and head content with related authors” 测试中，把 `blameFileRange` 和 `getCommit` 改为：

```ts
        blameFileRange: vi.fn(async () => [
          {
            commitHash: 'alice',
            originalLine: 1,
            finalLine: 1,
            authorName: 'Alice',
            authorEmail: 'a@example.com',
            committedAt: '2026-05-10T00:00:00.000Z',
            summary: 'feature: update app',
            content: 'const name = "new";'
          }
        ]),
        listParents: vi.fn(async () => []),
        getCommit: vi.fn(async () => commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app'))
```

断言增加：

```ts
    expect(overlay.blocks[0].attribution?.confidence).toBe('precise');
    expect(overlay.blocks[0].relatedCommits[0].attribution?.method).toBe('blame');
```

- [ ] **Step 2: 运行 IPC 测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/reviewIpc.test.ts
```

预期：失败，`reviewIpc` 尚未调用 analysis-core 或接口类型不匹配。

- [ ] **Step 3: 修改 reviewIpc 的 Git 接口**

修改 `src/main/ipc/reviewIpc.ts`，把本地 `AnalysisGitClient` 接口替换为从 core 导入：

```ts
import { buildFileOverlay } from '../../analysis-core';
import type { AnalysisGitClient } from '../../analysis-core';
```

删除 `attachAttribution`、`buildFileOverlayDiff`、`parsePatchTouchedRanges` 的导入，保留 commit overlay 仍需要的函数，直到后续单独迁移提交下钻。

- [ ] **Step 4: 使用 analysis-core 构建文件 overlay**

在 `buildFileOverlayForTask` 中，保留二进制分支，非二进制分支替换为：

```ts
  const overlay = await buildFileOverlay({
    repoPath: project.repoPath,
    file,
    range,
    rangeCommits,
    filters,
    git
  });
  const visibleBlocks = hasDisplayCommitFilters(filters)
    ? overlay.blocks.filter((block) => block.relatedCommits.some((commit) => commit.matchedByFilter))
    : overlay.blocks;

  return {
    ...overlay,
    rows: maskFilteredRows(overlay.rows ?? [], visibleBlocks),
    blocks: visibleBlocks
  };
```

保留 `buildCommitOverlayForTask` 的现有逻辑，只为生成的 commit block 增加：

```ts
      attribution: {
        confidence: 'precise',
        warnings: []
      }
```

并为 `relatedCommit` 增加：

```ts
    attribution: {
      method: 'blame',
      viaMergeHashes: []
    }
```

- [ ] **Step 5: 运行 IPC 测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/reviewIpc.test.ts
```

预期：通过。

- [ ] **Step 6: 提交**

运行：

```bash
git add src/main/ipc/reviewIpc.ts tests/unit/reviewIpc.test.ts
git commit -m "feat: IPC 接入分析核心归因"
```

## 任务 11：接入 UI 叹号提示

**Files:**
- Modify: `src/renderer/components/review/DiffBlockAuthors.vue`
- Modify: `src/renderer/components/review/DiffViewer.vue`
- Modify: `src/renderer/components/review/BlockDetailPanel.vue`
- Modify: `tests/unit/diffViewer.test.ts`
- Modify: `tests/unit/blockDetailPanel.test.ts`

- [ ] **Step 1: 写作者组件测试**

修改 `tests/unit/diffViewer.test.ts` 中的 block fixture，给 block 增加：

```ts
      attribution: {
        confidence: 'inferred',
        warnings: [
          {
            code: 'BLAME_UNAVAILABLE',
            message: '无法使用 blame 结果，已根据提交差异推断'
          }
        ]
      },
```

增加断言：

```ts
    expect(wrapper.text()).toContain('!');
```

如果现有测试没有 mount tooltip stub，则给 `n-tooltip` stub 使用：

```ts
          'n-tooltip': {
            template: '<span><slot name="trigger" /><slot /></span>'
          }
```

- [ ] **Step 2: 运行 UI 测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/diffViewer.test.ts tests/unit/blockDetailPanel.test.ts
```

预期：失败，作者组件不接收 attribution，也不显示叹号。

- [ ] **Step 3: 修改 DiffBlockAuthors**

修改 `src/renderer/components/review/DiffBlockAuthors.vue`：

```vue
<script setup lang="ts">
import type { AuthorSummary, BlockAttributionSummary } from '../../../shared/reviewTypes';

defineProps<{
  authors: AuthorSummary[];
  attribution?: BlockAttributionSummary;
}>();
</script>

<template>
  <div class="diff-authors">
    <n-tag v-if="authors.length === 0" size="small" :bordered="false">未知作者</n-tag>
    <n-tooltip
      v-for="author in authors"
      v-else
      :key="`${author.name}-${author.email ?? ''}`"
      placement="top"
    >
      <template #trigger>
        <n-tag size="small" :bordered="false">{{ author.name }}</n-tag>
      </template>
      {{ author.email || author.name }}
    </n-tooltip>
    <n-tooltip
      v-if="attribution?.confidence === 'inferred' || attribution?.confidence === 'partial'"
      placement="top"
    >
      <template #trigger>
        <n-tag size="small" type="warning" :bordered="false" aria-label="归因可信度较低">!</n-tag>
      </template>
      {{ attribution.warnings[0]?.message || '归因可信度较低，已根据提交差异推断' }}
    </n-tooltip>
  </div>
</template>
```

- [ ] **Step 4: 修改 DiffViewer 传递 attribution**

在 `src/renderer/components/review/DiffViewer.vue` 的 `DiffBlockAuthors` 调用处增加：

```vue
              :attribution="blockForRow(row)?.attribution"
```

- [ ] **Step 5: 修改详情面板显示提交来源**

在 `src/renderer/components/review/BlockDetailPanel.vue` 的相关提交条目中，作者时间行附近增加：

```vue
              <span v-if="commit.attribution?.viaMergeHashes.length">
                经由 merge 提交 {{ commit.attribution.viaMergeHashes.map((hash) => hash.slice(0, 8)).join(', ') }} 引入
              </span>
```

在作者 section 中：

```vue
        <DiffBlockAuthors :authors="block.authors" :attribution="block.attribution" />
```

- [ ] **Step 6: 运行 UI 测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/diffViewer.test.ts tests/unit/blockDetailPanel.test.ts
```

预期：通过，测试文本中不出现备用实现相关措辞。

- [ ] **Step 7: 提交**

运行：

```bash
git add src/renderer/components/review/DiffBlockAuthors.vue src/renderer/components/review/DiffViewer.vue src/renderer/components/review/BlockDetailPanel.vue tests/unit/diffViewer.test.ts tests/unit/blockDetailPanel.test.ts
git commit -m "feat: 显示归因可信度提示"
```

## 任务 12：完善 CLI 真实输出和 trace-block

**Files:**
- Modify: `src/cli/revier-analysis.ts`
- Test: `tests/integration/mergeAttribution.test.ts`

- [ ] **Step 1: 增加 CLI 输出集成断言**

追加到 `tests/integration/mergeAttribution.test.ts`：

```ts
import { spawnSync } from 'node:child_process';

describe('revier-analysis cli integration', () => {
  it('prints file overlay JSON without starting Electron', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-cli-overlay-'));

    try {
      const repo = await initTestRepo(dir);
      const base = await commitFiles(repo, {
        message: 'base cli app',
        files: { 'src/app.ts': 'export const value = 1;\n' }
      });
      const head = await commitFiles(repo, {
        message: 'feat: cli app',
        authorName: 'Alice',
        authorEmail: 'alice@example.com',
        files: { 'src/app.ts': 'export const value = 2;\n' }
      });

      const build = spawnSync('pnpm', ['build:analysis-cli'], {
        cwd: process.cwd(),
        encoding: 'utf8',
        shell: true
      });
      expect(build.status).toBe(0);

      const run = spawnSync(
        'node',
        [
          'dist-cli/src/cli/revier-analysis.js',
          'file-overlay',
          '--repo',
          dir,
          '--base',
          base,
          '--head',
          head,
          '--file',
          'src/app.ts',
          '--format',
          'json'
        ],
        {
          cwd: process.cwd(),
          encoding: 'utf8',
          shell: true
        }
      );

      expect(run.status).toBe(0);
      const overlay = JSON.parse(run.stdout);
      expect(overlay.blocks[0].authors[0].name).toBe('Alice');
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
```

- [ ] **Step 2: 运行 CLI 集成测试确认失败或不完整**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/integration/mergeAttribution.test.ts
```

预期：如果 CLI 构造 file 时 additions/deletions 为 0 但 overlay 仍可生成，则可能通过；如果缺少真实 changed file 元数据或 GitService 接口不完整，会失败。

- [ ] **Step 3: 完善 CLI 获取文件元数据**

修改 `src/cli/revier-analysis.ts`，在 `file-overlay` 和 `trace-block` 分支中，通过 `listChangedFiles` 查找目标文件：

```ts
    const files = await git.listChangedFiles(parsed.repoPath, parsed.baseCommit, parsed.headCommit);
    const file = files.find((item) => item.path === parsed.filePath || item.oldPath === parsed.filePath);
    if (!file) {
      throw new Error('没有可分析的变更');
    }
```

替换原先手写 `ChangedFile` 的代码。`trace-block` 输出时查找块：

```ts
    if (parsed.command === 'trace-block') {
      const block = overlay.blocks.find((item) => item.id === parsed.blockId);
      if (!block) {
        throw new Error(`未找到变更块 ${parsed.blockId}`);
      }
      process.stdout.write(JSON.stringify({ version: 1, overlay, blockId: parsed.blockId, block }, null, parsed.pretty ? 2 : 0));
      process.stdout.write('\n');
      return;
    }
```

- [ ] **Step 4: 运行 CLI 集成测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/integration/mergeAttribution.test.ts
```

预期：通过。

- [ ] **Step 5: 提交**

运行：

```bash
git add src/cli/revier-analysis.ts tests/integration/mergeAttribution.test.ts
git commit -m "feat: 完善分析 CLI 输出"
```

## 任务 13：完整验证与文档收口

**Files:**
- Verify: all changed files
- Modify if needed: `README.md`

- [ ] **Step 1: 运行目标单元测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/analysisCoreTypes.test.ts tests/unit/blameParser.test.ts tests/unit/analysisCli.test.ts tests/unit/attributionService.test.ts tests/unit/reviewIpc.test.ts tests/unit/diffViewer.test.ts tests/unit/blockDetailPanel.test.ts
```

预期：全部通过。

- [ ] **Step 2: 运行目标集成测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/integration/gitService.test.ts tests/integration/mergeAttribution.test.ts
```

预期：全部通过。

- [ ] **Step 3: 运行 CLI 编译**

运行：

```bash
fnm exec --using 24 pnpm build:analysis-cli
```

预期：通过。

- [ ] **Step 4: 运行类型检查**

运行：

```bash
fnm exec --using 24 pnpm typecheck
```

预期：通过。

- [ ] **Step 5: 运行完整测试**

运行：

```bash
fnm exec --using 24 pnpm test
```

预期：通过。

- [ ] **Step 6: 检查 UI 文案**

人工检查本次新增或修改的 UI 文案，确认只使用“归因可信度较低，已根据提交差异推断”等用户可理解表述，不出现备用实现相关措辞。

- [ ] **Step 7: 检查改动范围**

运行：

```bash
git status --short
git diff --stat
```

预期：只包含本计划列出的实现、测试和必要脚本文件。

- [ ] **Step 8: 最终提交**

如果前面每个任务都已单独提交，本步骤只提交 README 或计划勾选状态更新。若没有额外文档改动，不创建空提交。

需要提交时运行：

```bash
git add README.md docs/superpowers/plans/2026-06-14-revier-merge-attribution-implementation.md
git commit -m "docs: 更新复杂 merge 归因说明"
```

## 执行注意事项

- 每个任务完成后先运行该任务列出的目标测试，再提交。
- 不要在同一提交中混入 UI、Git fixture 和核心算法以外的重构。
- 删除块归因比新增和修改更容易产生歧义；先保证新增和修改 merge 链路通过，再增强删除块。
- 如果 `GitService` 和 `SimpleGitAnalysisClient` 出现循环依赖，优先让 `GitService` 组合 `SimpleGitAnalysisClient`，不要让 analysis-core 依赖 Electron。
- 如果 CLI 编译输出路径因 `tsconfig.cli.json` 变化不同，先更新 package 脚本和 CLI 集成测试，使两者保持一致。
