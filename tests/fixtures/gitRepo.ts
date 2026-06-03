import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import simpleGit, { type SimpleGit } from 'simple-git';

export interface TestRepo {
  path: string;
  git: SimpleGit;
}

export interface CommitOptions {
  authorName?: string;
  authorEmail?: string;
  message: string;
  date?: string;
  files: Record<string, string | Buffer>;
}

export async function initTestRepo(path: string): Promise<TestRepo> {
  await mkdir(path, { recursive: true });
  const git = simpleGit(path);
  await git.init();
  await git.addConfig('user.name', 'Test User');
  await git.addConfig('user.email', 'test@example.com');
  await writeFile(join(path, 'README.md'), '# demo\n', 'utf8');
  await git.add('.');
  await git.commit('initial commit');
  return { path, git };
}

export async function commitFiles(repo: TestRepo, options: CommitOptions): Promise<string> {
  for (const [relativePath, content] of Object.entries(options.files)) {
    const fullPath = join(repo.path, relativePath);
    await mkdir(dirname(fullPath), { recursive: true });
    await writeFile(fullPath, content);
  }

  await repo.git.add('.');
  const env = Object.fromEntries(
    Object.entries({
      GIT_AUTHOR_DATE: options.date,
      GIT_COMMITTER_DATE: options.date,
      GIT_AUTHOR_NAME: options.authorName,
      GIT_AUTHOR_EMAIL: options.authorEmail
    }).filter((entry): entry is [string, string] => entry[1] !== undefined)
  );
  await repo.git.env(env).commit(options.message);
  return (await repo.git.revparse(['HEAD'])).trim();
}
