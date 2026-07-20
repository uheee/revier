import type { AuthorSummary, DiffBlock } from '../generated/bindings';

export const AUTHOR_RAIL_WIDTH = 112;
export const AUTHOR_ROW_HEIGHT = 20;
export const AUTHOR_ROW_GAP = 2;
export const AUTHOR_POPOVER_MAX_HEIGHT = 320;

export interface FittedAuthors {
  visible: AuthorSummary[];
  hasMore: boolean;
}

export interface DiffBlockGeometry {
  top: number;
  height: number;
}

interface VisibleLineRange {
  startLineNumber: number;
  endLineNumber: number;
}

export interface DiffBlockGeometryEditor {
  getVisibleRanges(): readonly VisibleLineRange[];
  getTopForLineNumber(lineNumber: number): number;
  getBottomForLineNumber(lineNumber: number): number;
  getScrolledVisiblePosition(position: { lineNumber: number; column: number }): { top: number; height: number } | null;
  getScrollTop(): number;
  getLayoutInfo(): { height: number };
}

function parsedTime(value: string): number {
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? timestamp : 0;
}

export function sortBlockAuthors(authors: readonly AuthorSummary[]): AuthorSummary[] {
  return [...authors].sort((left, right) => (
    right.commitCount - left.commitCount
    || parsedTime(right.lastCommittedAt) - parsedTime(left.lastCommittedAt)
    || left.name.localeCompare(right.name, 'zh-CN')
  ));
}

/** 保留传入顺序；调用方应先按块作者规则完成排序。 */
export function fitAuthors(height: number, sortedAuthors: readonly AuthorSummary[]): FittedAuthors {
  const capacity = Math.max(0, Math.floor((height + AUTHOR_ROW_GAP) / (AUTHOR_ROW_HEIGHT + AUTHOR_ROW_GAP)));
  if (sortedAuthors.length <= capacity) {
    return { visible: [...sortedAuthors], hasMore: false };
  }
  return {
    visible: sortedAuthors.slice(0, Math.max(0, capacity - 1)),
    hasMore: sortedAuthors.length > 0
  };
}

function finiteNonNegative(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

function lineTop(editorInstance: DiffBlockGeometryEditor, lineNumber: number): number {
  const visiblePosition = editorInstance.getScrolledVisiblePosition({ lineNumber, column: 1 });
  const fallback = editorInstance.getTopForLineNumber(lineNumber) - editorInstance.getScrollTop();
  return Number.isFinite(visiblePosition?.top) ? visiblePosition!.top : fallback;
}

function lineBottom(editorInstance: DiffBlockGeometryEditor, lineNumber: number): number {
  return editorInstance.getBottomForLineNumber(lineNumber) - editorInstance.getScrollTop();
}

export function getDiffBlockGeometry(
  block: DiffBlock,
  originalEditor: DiffBlockGeometryEditor,
  modifiedEditor: DiffBlockGeometryEditor
): DiffBlockGeometry | undefined {
  if (block.changeType === 'modified') {
    const original = sideGeometry(block.oldStart, block.oldEnd, originalEditor);
    const modified = sideGeometry(block.newStart, block.newEnd, modifiedEditor);
    if (!original) return modified;
    if (!modified) return original;
    const top = Math.min(original.top, modified.top);
    const bottom = Math.max(original.top + original.height, modified.top + modified.height);
    return { top, height: bottom - top };
  }
  const usesOriginal = block.changeType === 'deleted';
  return sideGeometry(
    usesOriginal ? block.oldStart : block.newStart,
    usesOriginal ? block.oldEnd : block.newEnd,
    usesOriginal ? originalEditor : modifiedEditor
  );
}

function sideGeometry(
  start: number,
  end: number,
  editorInstance: DiffBlockGeometryEditor
): DiffBlockGeometry | undefined {
  if (start <= 0 || end < start) {
    return undefined;
  }

  const intersections = editorInstance.getVisibleRanges()
    .map((range) => ({
      start: Math.max(start, range.startLineNumber),
      end: Math.min(end, range.endLineNumber)
    }))
    .filter((range) => range.start <= range.end);
  if (intersections.length === 0) {
    return undefined;
  }

  const layoutHeight = finiteNonNegative(editorInstance.getLayoutInfo().height);
  if (layoutHeight === 0) {
    return undefined;
  }

  const rawTop = Math.min(...intersections.map((range) => lineTop(editorInstance, range.start)));
  const rawBottom = Math.max(...intersections.map((range) => lineBottom(editorInstance, range.end)));
  if (!Number.isFinite(rawTop) || !Number.isFinite(rawBottom) || rawBottom <= 0 || rawTop >= layoutHeight) {
    return undefined;
  }

  const top = Math.max(0, rawTop);
  const bottom = Math.min(layoutHeight, rawBottom);
  const height = finiteNonNegative(bottom - top);
  return height > 0 ? { top, height } : undefined;
}
