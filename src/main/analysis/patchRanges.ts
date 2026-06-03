import type { TouchedRange } from '../../shared/reviewTypes';

const hunkPattern = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/gm;

export function parsePatchTouchedRanges(patch: string): TouchedRange[] {
  const ranges: TouchedRange[] = [];
  for (const match of patch.matchAll(hunkPattern)) {
    const oldStart = Number(match[1]);
    const oldLength = Number(match[2] ?? '1');
    const newStart = Number(match[3]);
    const newLength = Number(match[4] ?? '1');
    ranges.push({
      oldStart,
      oldEnd: oldStart + Math.max(oldLength - 1, 0),
      newStart,
      newEnd: newStart + Math.max(newLength - 1, 0)
    });
  }
  return ranges;
}

export function rangesIntersect(block: TouchedRange, touched: TouchedRange): boolean {
  return (
    intersects(block.oldStart, block.oldEnd, touched.oldStart, touched.oldEnd) ||
    intersects(block.newStart, block.newEnd, touched.newStart, touched.newEnd)
  );
}

function intersects(
  leftStart?: number,
  leftEnd?: number,
  rightStart?: number,
  rightEnd?: number
): boolean {
  if (
    leftStart === undefined ||
    leftEnd === undefined ||
    rightStart === undefined ||
    rightEnd === undefined
  ) {
    return false;
  }
  return leftStart <= rightEnd && rightStart <= leftEnd;
}
