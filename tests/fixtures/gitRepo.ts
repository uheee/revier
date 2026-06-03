import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import simpleGit, { type SimpleGit } from 'simple-git';

export interface TestRepo {
  path: string;
  git: SimpleGit;
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
