import { createAnalysisCache } from '../../src/analysis-core';
import type { AnalyzeRequest, AnalysisCliResult } from '../../src/analysis-core';
import type { DiffBlock, RelatedCommit } from '../../src/shared/reviewTypes';

describe('analysis attribution shared types', () => {
  it('supports block attribution confidence and related commit provenance', () => {
    const relatedCommit: RelatedCommit = {
      hash: 'a'.repeat(40), shortHash: 'aaaaaaaa', authorName: 'Alice', authorEmail: 'alice@example.com', committedAt: '2026-06-01T00:00:00.000Z', subject: 'feat: 原始提交', matchedByFilter: false, touchedRanges: [], attribution: { method: 'merge-trace', viaMergeHashes: ['b'.repeat(40)] }
    };
    const block: DiffBlock = { id: 'block-1', oldStart: 1, oldEnd: 1, newStart: 1, newEnd: 1, changeType: 'modified', authors: [{ name: 'Alice', email: 'alice@example.com' }], rows: [], relatedCommits: [relatedCommit], attribution: { confidence: 'partial', warnings: [{ code: 'MERGE_TRACE_AMBIGUOUS', message: '部分行存在多条可能的 merge 来源' }] } };
    expect(block.attribution?.confidence).toBe('partial');
    expect(block.relatedCommits[0].attribution?.method).toBe('merge-trace');
    expect(block.relatedCommits[0].attribution?.viaMergeHashes).toHaveLength(1);
  });
});

describe('analysis-core contracts', () => {
  it('exports request, result, and cache contracts without Electron dependencies', () => {
    const request: AnalyzeRequest = { repoPath: 'E:/repo/app', baseCommit: 'base', headCommit: 'head', globRules: ['src/**/*.ts'] };
    const result: AnalysisCliResult = { version: 1, range: { baseCommit: request.baseCommit, headCommit: request.headCommit }, files: [], overlays: [], warnings: [] };
    const cache = createAnalysisCache(); cache.parents.set('merge', ['left', 'right']);
    expect(result.version).toBe(1); expect(cache.parents.get('merge')).toEqual(['left', 'right']);
  });
});
