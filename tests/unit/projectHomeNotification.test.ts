import { mount } from '@vue/test-utils';
import type { Ref } from 'vue';
import ProjectHome from '../../src/renderer/pages/ProjectHome.vue';
import type { ReviewProject } from '../../src/renderer/generated/bindings';

const harness = vi.hoisted(() => ({
  loadProjects: vi.fn(),
  push: vi.fn(),
  projects: { __v_isRef: true, value: [] } as unknown as Ref<ReviewProject[]>,
  loading: { __v_isRef: true, value: false } as unknown as Ref<boolean>,
  error: { __v_isRef: true, value: undefined } as unknown as Ref<string | undefined>
}));

vi.mock('pinia', async (importOriginal) => {
  const actual = await importOriginal<typeof import('pinia')>();
  return {
    ...actual,
    storeToRefs: () => ({
      projects: harness.projects,
      loading: harness.loading,
      error: harness.error
    })
  };
});
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: harness.push })
}));
vi.mock('../../src/renderer/stores/projectStore', () => ({
  useProjectStore: () => ({
    error: undefined,
    loadProjects: harness.loadProjects,
    addProject: vi.fn(),
    removeProject: vi.fn()
  })
}));

describe('项目页通知入口', () => {
  beforeEach(() => {
    harness.loadProjects.mockReset();
    harness.push.mockReset();
    harness.projects.value = [];
    harness.loading.value = false;
    harness.error.value = undefined;
  });

  it('动作区严格按通知、刷新、打开项目排列', () => {
    const wrapper = mount(ProjectHome, {
      global: {
        stubs: {
          NotificationCenter: {
            name: 'NotificationCenter',
            template: '<button data-test="notification-center">通知</button>'
          },
          NButton: { template: '<button class="n-button"><slot /></button>' },
          NAlert: true,
          NModal: true,
          ProjectEditor: true,
          ProjectList: true,
          FolderOpen: true
        }
      }
    });

    const actions = wrapper.get('.project-topbar__actions');
    const children = Array.from(actions.element.children) as HTMLElement[];
    expect(wrapper.findAll('[data-test="notification-center"]')).toHaveLength(1);
    expect(children).toHaveLength(3);
    expect(children[0]?.dataset.test).toBe('notification-center');
    expect(children[1]?.textContent?.trim()).toBe('刷新');
    expect(children[2]?.textContent?.trim()).toBe('打开项目');
  });
});
