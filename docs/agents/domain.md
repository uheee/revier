# Domain Docs

本文档说明 engineering skills 在探索本仓库代码时应如何读取领域文档。

## 探索前优先读取

- 根目录的 `CONTEXT.md`；或者
- 根目录的 `CONTEXT-MAP.md`，如果它存在。该文件会指向每个上下文对应的 `CONTEXT.md`，只读取与当前任务相关的上下文；以及
- `docs/adr/` 中与当前工作区域相关的 ADR。在多上下文仓库中，还应检查 `src/<context>/docs/adr/` 下的上下文级决策。

如果这些文件不存在，直接继续探索即可。不要因为缺少这些文件而报错，也不要在任务开始时主动建议创建它们。`/domain-modeling` skill 会在术语或架构决策真正需要沉淀时按需创建。

## 文件结构

本仓库采用单上下文布局：

```text
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-example-decision.md
│   └── 0002-another-example-decision.md
└── src/
```

如未来演进为多上下文布局，可在根目录引入 `CONTEXT-MAP.md`：

```text
/
├── CONTEXT-MAP.md
├── docs/adr/
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 使用 glossary 中的词汇

当输出中需要命名领域概念时，例如 issue 标题、重构提案、假设、测试名，应优先使用 `CONTEXT.md` 中定义的术语。不要随意改用 glossary 明确避免的同义词。

如果需要的概念还没有出现在 glossary 中，这通常是一个信号：要么当前表述并非项目实际使用的语言，需要重新考虑；要么确实存在术语缺口，可以记录给 `/domain-modeling` 后续处理。

## 标记 ADR 冲突

如果输出内容和已有 ADR 冲突，需要明确指出，而不是静默覆盖。

示例：

> Contradicts ADR-0007 (event-sourced orders), but worth reopening because...
