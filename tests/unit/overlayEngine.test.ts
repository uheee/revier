import { buildFileOverlayBlocks, buildFileOverlayDiff } from '../../src/main/analysis/overlayEngine';
import type { ChangedFile } from '../../src/shared/reviewTypes';

const file: ChangedFile = {
  path: 'src/app.ts',
  status: 'modified',
  additions: 1,
  deletions: 1,
  isBinary: false,
  isPreviewable: true
};

describe('overlayEngine', () => {
  it('keeps context rows while marking changed rows with block ids', () => {
    const diff = buildFileOverlayDiff({
      file,
      oldText: 'same before\nconst name = "old";\nsame after\n',
      newText: 'same before\nconst name = "new";\nsame after\n'
    });

    expect(diff.rows).toHaveLength(3);
    expect(diff.blocks).toHaveLength(1);
    expect(diff.rows[0].type).toBe('context');
    expect(diff.rows[1].type).toBe('modified');
    expect(diff.rows[1].blockId).toBe(diff.blocks[0].id);
    expect(diff.rows[2].type).toBe('context');
    expect(diff.blocks[0].rowStartIndex).toBe(1);
    expect(diff.blocks[0].rowEndIndex).toBe(1);
  });

  it('keeps empty placeholders for added and deleted lines in full rows', () => {
    const diff = buildFileOverlayDiff({
      file,
      oldText: 'one\nthree\n',
      newText: 'one\ntwo\nthree\n'
    });

    const added = diff.rows.find((row) => row.type === 'added');
    expect(added?.oldLineNumber).toBeUndefined();
    expect(added?.newText).toBe('two');
    expect(added?.blockId).toBe('block-1');
  });

  it('creates side-by-side modified blocks with word changes', () => {
    const blocks = buildFileOverlayBlocks({
      file,
      oldText: 'const name = "old";\nconsole.log(name);\n',
      newText: 'const name = "new";\nconsole.log(name);\n'
    });

    expect(blocks).toHaveLength(1);
    expect(blocks[0].changeType).toBe('modified');
    expect(blocks[0].rows[0].type).toBe('modified');
    expect(blocks[0].rows[0].wordChanges?.some((change) => change.added)).toBe(true);
  });

  it('groups adjacent added lines into one block', () => {
    const blocks = buildFileOverlayBlocks({
      file: { ...file, status: 'added' },
      oldText: '',
      newText: 'one\ntwo\n'
    });

    expect(blocks).toHaveLength(1);
    expect(blocks[0].newStart).toBe(1);
    expect(blocks[0].newEnd).toBe(2);
  });
});
