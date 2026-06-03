import { parseNameStatus, parseNumstat } from '../../src/main/analysis/changedFiles';

describe('changedFiles', () => {
  it('parses added, modified, deleted, renamed, and binary files', () => {
    const files = parseNameStatus(
      ['A\tsrc/new.ts', 'M\tsrc/app.ts', 'D\tsrc/old.ts', 'R100\tsrc/a.ts\tsrc/b.ts'].join(
        '\n'
      )
    );
    const withStats = parseNumstat(
      files,
      ['4\t0\tsrc/new.ts', '2\t1\tsrc/app.ts', '-\t-\tassets/logo.png'].join('\n')
    );

    expect(withStats.find((file) => file.path === 'src/b.ts')?.oldPath).toBe('src/a.ts');
    expect(withStats.find((file) => file.path === 'assets/logo.png')?.isBinary).toBe(true);
  });
});
