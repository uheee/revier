import { diffLines, diffWords } from 'diff';
import type { ChangedFile, DiffBlock, SideBySideDiffRow } from '../../shared/reviewTypes';

export interface BuildOverlayInput {
  file: ChangedFile;
  oldText: string;
  newText: string;
}

export interface BuiltFileOverlayDiff {
  rows: SideBySideDiffRow[];
  blocks: DiffBlock[];
}

export function buildFileOverlayDiff(input: BuildOverlayInput): BuiltFileOverlayDiff {
  const rows = buildSideBySideRows(input.oldText, input.newText);
  return {
    rows,
    blocks: groupChangedRows(rows)
  };
}

export function buildFileOverlayBlocks(input: BuildOverlayInput): DiffBlock[] {
  return buildFileOverlayDiff(input).blocks;
}

function buildSideBySideRows(oldText: string, newText: string): SideBySideDiffRow[] {
  const parts = diffLines(oldText, newText, { newlineIsToken: false });
  const rows: SideBySideDiffRow[] = [];
  let oldLine = 1;
  let newLine = 1;

  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index];
    const next = parts[index + 1];
    if (part.removed && next?.added) {
      const oldLines = splitLines(part.value);
      const newLines = splitLines(next.value);
      const max = Math.max(oldLines.length, newLines.length);

      for (let offset = 0; offset < max; offset += 1) {
        const oldText = oldLines[offset];
        const newText = newLines[offset];
        rows.push({
          oldLineNumber: oldText === undefined ? undefined : oldLine++,
          newLineNumber: newText === undefined ? undefined : newLine++,
          oldText,
          newText,
          type: 'modified',
          wordChanges:
            oldText !== undefined && newText !== undefined ? diffWords(oldText, newText) : undefined
        });
      }
      index += 1;
      continue;
    }

    for (const line of splitLines(part.value)) {
      if (part.added) {
        rows.push({ newLineNumber: newLine++, newText: line, type: 'added' });
      } else if (part.removed) {
        rows.push({ oldLineNumber: oldLine++, oldText: line, type: 'deleted' });
      } else {
        rows.push({
          oldLineNumber: oldLine++,
          newLineNumber: newLine++,
          oldText: line,
          newText: line,
          type: 'context'
        });
      }
    }
  }

  return rows;
}

function splitLines(value: string): string[] {
  if (value.length === 0) {
    return [];
  }
  return value.endsWith('\n') ? value.slice(0, -1).split('\n') : value.split('\n');
}

function groupChangedRows(rows: SideBySideDiffRow[]): DiffBlock[] {
  const blocks: DiffBlock[] = [];
  let current: Array<{ row: SideBySideDiffRow; index: number }> = [];

  const flush = () => {
    if (current.length === 0) {
      return;
    }

    const blockId = `block-${blocks.length + 1}`;
    for (const item of current) {
      item.row.blockId = blockId;
    }

    const blockRows = current.map((item) => item.row);
    const oldNumbers = blockRows
      .map((row) => row.oldLineNumber)
      .filter((line): line is number => line !== undefined);
    const newNumbers = blockRows
      .map((row) => row.newLineNumber)
      .filter((line): line is number => line !== undefined);
    const changeTypes = new Set(blockRows.map((row) => row.type));
    blocks.push({
      id: blockId,
      oldStart: oldNumbers[0] ?? 0,
      oldEnd: oldNumbers.at(-1) ?? 0,
      newStart: newNumbers[0] ?? 0,
      newEnd: newNumbers.at(-1) ?? 0,
      rowStartIndex: current[0].index,
      rowEndIndex: current.at(-1)?.index ?? current[0].index,
      changeType: changeTypes.has('modified')
        ? 'modified'
        : changeTypes.has('added')
          ? 'added'
          : 'deleted',
      authors: [],
      rows: blockRows,
      relatedCommits: []
    });
    current = [];
  };

  rows.forEach((row, index) => {
    if (row.type === 'context') {
      flush();
    } else {
      current.push({ row, index });
    }
  });
  flush();
  return blocks;
}
