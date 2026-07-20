import type { editor } from 'monaco-editor';
import type {
  DiffBlock,
  DiffBlockAttribution,
  DiffBlockChangeType,
  DiffBlockRange,
  RelatedCommit
} from '../generated/bindings';

export type MonacoCanonicalBlock = DiffBlock & {
  oldAnchor?: number;
  newAnchor?: number;
  order: number;
};

export interface MonacoDiffBlocksPayload {
  generation: number;
  contextKey: string | number;
  signature: string;
  blocks: MonacoCanonicalBlock[];
}

interface LineChangeLike {
  originalStartLineNumber: number;
  originalEndLineNumber: number;
  modifiedStartLineNumber: number;
  modifiedEndLineNumber: number;
}

function modelLineCount(model: editor.ITextModel): number {
  return model.getLineCount();
}

function isEmptyModel(model: editor.ITextModel): boolean {
  return model.getValueLength() === 0;
}

function normalizeSide(
  start: number,
  end: number,
  lineCount: number,
  emptyModel: boolean
): { start: number; end: number; anchor?: number } {
  if (end === 0) {
    return { start: 0, end: 0, anchor: start };
  }
  if (emptyModel && start === 1 && end === 1) {
    return { start: 0, end: 0, anchor: 1 };
  }
  if (start < 1 || end < start || end > lineCount) {
    throw new Error(`Monaco diff 行范围越界：${start}-${end} / ${lineCount}`);
  }
  return { start, end };
}

function changeType(
  oldStart: number,
  oldEnd: number,
  newStart: number,
  newEnd: number
): DiffBlockChangeType {
  const hasOld = oldStart > 0 && oldEnd > 0;
  const hasNew = newStart > 0 && newEnd > 0;
  if (!hasOld && hasNew) return 'added';
  if (hasOld && !hasNew) return 'deleted';
  if (hasOld && hasNew) return 'modified';
  throw new Error('Monaco diff 块两侧不能同时为空');
}

function emptyRows(): DiffBlock['rows'] {
  return [];
}

function emptyCommits(): RelatedCommit[] {
  return [];
}

export function normalizeMonacoDiffBlocks(
  changes: readonly LineChangeLike[] | null,
  originalModel: editor.ITextModel,
  modifiedModel: editor.ITextModel
): MonacoCanonicalBlock[] | null {
  if (changes === null) {
    return null;
  }

  const oldLineCount = modelLineCount(originalModel);
  const newLineCount = modelLineCount(modifiedModel);
  const oldEmpty = isEmptyModel(originalModel);
  const newEmpty = isEmptyModel(modifiedModel);
  const seen = new Set<string>();

  return [...changes]
    .sort((left, right) => (
      left.modifiedStartLineNumber - right.modifiedStartLineNumber
      || left.originalStartLineNumber - right.originalStartLineNumber
    ))
    .map((change, index) => {
      const oldSide = normalizeSide(
        change.originalStartLineNumber,
        change.originalEndLineNumber,
        oldLineCount,
        oldEmpty
      );
      const newSide = normalizeSide(
        change.modifiedStartLineNumber,
        change.modifiedEndLineNumber,
        newLineCount,
        newEmpty
      );
      const id = `monaco:${oldSide.start}-${oldSide.end}:${newSide.start}-${newSide.end}`;
      if (!seen.add(id)) {
        throw new Error(`Monaco diff 块 ID 重复：${id}`);
      }
      const block: MonacoCanonicalBlock = {
        id,
        oldStart: oldSide.start,
        oldEnd: oldSide.end,
        newStart: newSide.start,
        newEnd: newSide.end,
        oldAnchor: oldSide.anchor,
        newAnchor: newSide.anchor,
        order: index,
        rowStartIndex: undefined,
        rowEndIndex: undefined,
        changeType: changeType(oldSide.start, oldSide.end, newSide.start, newSide.end),
        authors: [],
        rows: emptyRows(),
        relatedCommits: emptyCommits(),
        attribution: undefined
      };
      return block;
    });
}

export function diffBlocksSignature(blocks: readonly DiffBlock[]): string {
  return blocks
    .map((block) => `${block.id}:${block.oldStart}-${block.oldEnd}:${block.newStart}-${block.newEnd}:${block.changeType}`)
    .join('|');
}

export function toDiffBlockRanges(blocks: readonly DiffBlock[]): DiffBlockRange[] {
  return blocks.map((block) => ({
    id: block.id,
    oldStart: block.oldStart,
    oldEnd: block.oldEnd,
    newStart: block.newStart,
    newEnd: block.newEnd,
    changeType: block.changeType
  }));
}

export function mergeDiffBlockAttributions(
  blocks: readonly DiffBlock[],
  attributions: readonly DiffBlockAttribution[]
): DiffBlock[] {
  const byId = new Map<string, DiffBlockAttribution>();
  for (const attribution of attributions) {
    if (byId.has(attribution.id)) {
      throw new Error(`归因结果包含重复块 ID：${attribution.id}`);
    }
    byId.set(attribution.id, attribution);
  }
  if (byId.size !== blocks.length) {
    throw new Error('归因结果块数量与 Monaco 块数量不一致');
  }
  return blocks.map((block) => {
    const attribution = byId.get(block.id);
    if (!attribution) {
      throw new Error(`归因结果缺少块 ID：${block.id}`);
    }
    return {
      ...block,
      authors: attribution.authors,
      relatedCommits: attribution.relatedCommits,
      attribution: attribution.attribution
    };
  });
}
