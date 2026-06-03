import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { JsonProjectStore } from '../../src/main/projects/projectStore';

describe('JsonProjectStore', () => {
  it('persists projects with repository preferences', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-projects-'));
    const file = join(dir, 'projects.json');

    try {
      const store = new JsonProjectStore(file);
      const project = await store.add('E:/repos/demo', {
        name: 'demo',
        pinned: true,
        preferences: {
          defaultBranch: 'main',
          defaultDays: 30,
          defaultGlobRules: ['src/**/*.ts', '!**/*.test.ts']
        }
      });

      const loaded = await store.list();
      const raw = JSON.parse(await readFile(file, 'utf8'));

      expect(loaded).toHaveLength(1);
      expect(loaded[0].id).toBe(project.id);
      expect(raw.projects[0].preferences.defaultDays).toBe(30);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it('updates and removes projects by id', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-projects-'));
    const file = join(dir, 'projects.json');

    try {
      const store = new JsonProjectStore(file);
      const project = await store.add('E:/repos/demo', { name: 'demo' });
      await store.update({ ...project, pinned: true });
      await store.remove(project.id);

      expect(await store.list()).toEqual([]);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it('rejects duplicate normalized repository paths', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-projects-'));
    const file = join(dir, 'projects.json');

    try {
      const store = new JsonProjectStore(file);
      await store.add('E:/repos/demo', { name: 'demo' });

      await expect(store.add('E:/repos/demo/', { name: 'again' })).rejects.toThrow(
        '该仓库已在项目列表中'
      );
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
