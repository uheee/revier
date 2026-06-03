import { buildFileOverlayBlocks } from '../../src/main/analysis/overlayEngine';
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
