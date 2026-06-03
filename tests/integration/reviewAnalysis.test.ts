import { mkdtemp, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createGlobMatcher } from '../../src/main/analysis/globRules';
import { buildFileOverlayBlocks } from '../../src/main/analysis/overlayEngine';
import { selectRangeCommits } from '../../src/main/analysis/rangeResolver';
import { GitService } from '../../src/main/git/gitService';
import { commitFiles, initTestRepo, type TestRepo } from '../fixtures/gitRepo';

describe('review analysis integration', () => {
  let repo: TestRepo;

  beforeEach(async () => {
    repo = await initTestRepo(await mkdtemp(join(tmpdir(), 'revier-review-')));
  });

  afterEach(async () => {
    await rm(repo.path, { recursive: true, force: true });
  });

  it('builds final overlay blocks from the selected range and glob-filtered files', async () => {
    const git = new GitService();
    const baseCommit = await commitFiles(repo, {
      message: 'add app files',
      date: '2026-05-01T00:00:00.000Z',
      files: {
        'src/app.ts': 'export const label = "old";\nconsole.log(label);\n',
        'src/app.test.ts': 'expect(label).toBe("old");\n'
      }
    });
    const headCommit = await commitFiles(repo, {
      message: 'update app files',
      date: '2026-05-10T00:00:00.000Z',
      files: {
        'src/app.ts': 'export const label = "new";\nconsole.log(label);\n',
        'src/app.test.ts': 'expect(label).toBe("new");\n'
      }
    });

    const commits = await git.listCommits(repo.path, 'HEAD');
    const range = selectRangeCommits(commits, {
      branch: 'HEAD',
      startAt: '2026-05-05T00:00:00.000Z',
      endAt: '2026-05-31T00:00:00.000Z'
    });
    const matcher = createGlobMatcher(['src/**/*.ts', '!**/*.test.ts']);
    const files = (await git.listChangedFiles(repo.path, range.baseCommit, range.headCommit)).filter(
      (file) => matcher(file.path)
    );

    const oldText = await git.readFileAtCommit(repo.path, range.baseCommit, 'src/app.ts');
    const newText = await git.readFileAtCommit(repo.path, range.headCommit, 'src/app.ts');
    const blocks = buildFileOverlayBlocks({ file: files[0], oldText, newText });

    expect(range.baseCommit).toBe(baseCommit);
    expect(range.headCommit).toBe(headCommit);
    expect(files.map((file) => file.path)).toEqual(['src/app.ts']);
    expect(oldText).toContain('"old"');
    expect(newText).toContain('"new"');
    expect(blocks[0].changeType).toBe('modified');
    expect(blocks[0].rows[0].wordChanges?.some((change) => change.added)).toBe(true);
  });
});
