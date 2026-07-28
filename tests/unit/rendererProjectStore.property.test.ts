import fc from 'fast-check';
import { createPinia, setActivePinia } from 'pinia';
import { useProjectStore } from '../../src/renderer/stores/projectStore';
import { revierClient } from '../../src/renderer/api/revierClient';
import type { ReviewProject } from '../../src/renderer/generated/bindings';

vi.mock('../../src/renderer/api/revierClient', () => ({
  revierClient: {
    projects: {
      list: vi.fn(),
      add: vi.fn(),
      update: vi.fn(),
      remove: vi.fn(),
      validateRepository: vi.fn(),
      listBranches: vi.fn(),
      selectDirectory: vi.fn()
    },
    review: {
      startAnalysis: vi.fn(),
      cancelAnalysis: vi.fn(),
      getTask: vi.fn(),
      onTaskUpdate: vi.fn(),
      listChangedFiles: vi.fn(),
      getFileOverlay: vi.fn(),
      getCommitOverlay: vi.fn(),
      listAuthors: vi.fn()
    }
  }
}));

describe('renderer projectStore property', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.mocked(revierClient.projects.list).mockReset();
  });

  it('乱序项目列表响应不会覆盖最新请求结果', async () => {
    await fc.assert(
      fc.asyncProperty(projectListArbitrary(), projectListArbitrary(), fc.boolean(), async (
        firstProjects,
        secondProjects,
        resolveFirstBeforeSecond
      ) => {
        setActivePinia(createPinia());
        vi.mocked(revierClient.projects.list).mockReset();
        const first = deferred<ReviewProject[]>();
        const second = deferred<ReviewProject[]>();
        vi.mocked(revierClient.projects.list)
          .mockReturnValueOnce(first.promise)
          .mockReturnValueOnce(second.promise);

        const store = useProjectStore();
        const firstLoad = store.loadProjects();
        const secondLoad = store.loadProjects();

        if (resolveFirstBeforeSecond) {
          first.resolve(firstProjects);
          await firstLoad;
          second.resolve(secondProjects);
        } else {
          second.resolve(secondProjects);
          await secondLoad;
          first.resolve(firstProjects);
        }
        await Promise.all([firstLoad, secondLoad]);

        expect(store.projects).toStrictEqual(secondProjects);
        expect(store.loading).toBe(false);
        expect(store.error).toBeUndefined();
      }),
      { numRuns: 50 }
    );
  });
});

function projectListArbitrary(): fc.Arbitrary<ReviewProject[]> {
  return fc.array(projectArbitrary(), { maxLength: 6 });
}

function projectArbitrary(): fc.Arbitrary<ReviewProject> {
  return fc.record({
    id: fc.string({ minLength: 1, maxLength: 12 }),
    name: fc.string({ minLength: 1, maxLength: 24 }),
    repoPath: fc.string({ minLength: 1, maxLength: 48 }),
    pinned: fc.boolean(),
    lastOpenedAt: fc.option(fc.string({ minLength: 1, maxLength: 32 }), { nil: undefined }),
    preferences: fc.record({
      defaultBranch: fc.option(fc.string({ minLength: 1, maxLength: 16 }), { nil: undefined }),
      defaultDays: fc.option(fc.integer({ min: 1, max: 365 }), { nil: undefined }),
      defaultGlobRules: fc.array(fc.string({ minLength: 1, maxLength: 24 }), { maxLength: 4 })
    })
  });
}

function deferred<T>(): {
  promise: Promise<T>;
  resolve: (value: T) => void;
} {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
