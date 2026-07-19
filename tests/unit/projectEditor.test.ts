// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils';
import ProjectEditor from '../../src/renderer/components/projects/ProjectEditor.vue';
import { revierClient } from '../../src/renderer/api/revierClient';
import { useNotifications } from '../../src/renderer/composables/useNotifications';

vi.mock('../../src/renderer/api/revierClient', () => ({
  revierClient: {
    projects: {
      selectDirectory: vi.fn()
    }
  }
}));

describe('ProjectEditor', () => {
  beforeEach(() => {
    vi.mocked(revierClient.projects.selectDirectory).mockReset();
    useNotifications().clear();
  });

  it('selects a directory and fills repo path and project name', async () => {
    vi.mocked(revierClient.projects.selectDirectory).mockResolvedValue({
      path: 'E:/Projects/revier',
      name: 'revier'
    });

    const wrapper = mount(ProjectEditor, {
      global: {
        stubs: {
          'n-input': {
            props: ['value'],
            emits: ['update:value'],
            template:
              '<input :value="value" @input="$emit(\'update:value\', $event.target.value)" />'
          },
          'n-button': {
            emits: ['click'],
            template: '<button type="button" @click="$emit(\'click\')"><slot /></button>'
          },
          'n-alert': {
            props: ['title'],
            template: '<div data-test="directory-error">{{ title }}</div>'
          }
        }
      }
    });

    await wrapper.get('[data-test="select-repo-directory"]').trigger('click');
    await flushPromises();
    const inputs = wrapper.findAll('input');

    expect(revierClient.projects.selectDirectory).toHaveBeenCalledTimes(1);
    expect((inputs[0].element as HTMLInputElement).value).toBe('E:/Projects/revier');
    expect((inputs[1].element as HTMLInputElement).value).toBe('revier');
  });

  it('shows a readable error when directory selection fails', async () => {
    vi.mocked(revierClient.projects.selectDirectory).mockRejectedValue({
      code: 'DIALOG_PATH_INVALID',
      message: '目录路径无法转换为本地路径'
    });

    const wrapper = mount(ProjectEditor, {
      global: {
        stubs: {
          'n-input': {
            props: ['value'],
            emits: ['update:value'],
            template:
              '<input :value="value" @input="$emit(\'update:value\', $event.target.value)" />'
          },
          'n-button': {
            emits: ['click'],
            template: '<button type="button" @click="$emit(\'click\')"><slot /></button>'
          },
          'n-alert': {
            props: ['title'],
            template: '<div data-test="directory-error">{{ title }}</div>'
          }
        }
      }
    });

    await wrapper.get('[data-test="select-repo-directory"]').trigger('click');
    await flushPromises();

    expect(wrapper.get('[data-test="directory-error"]').text()).toBe(
      '目录路径无法转换为本地路径'
    );
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '仓库目录选择失败',
        message: '目录路径无法转换为本地路径',
        source: 'Projects'
      })
    ]);
  });

  it('旧目录选择失败不得覆盖新选择或产生陈旧通知', async () => {
    let rejectOld!: (reason: unknown) => void;
    vi.mocked(revierClient.projects.selectDirectory)
      .mockReturnValueOnce(new Promise((_, reject) => { rejectOld = reject; }))
      .mockResolvedValueOnce({ path: 'E:/Projects/new', name: 'new' });

    const wrapper = mount(ProjectEditor, {
      global: { stubs: {
        'n-input': {
          props: ['value'], emits: ['update:value'],
          template: '<input :value="value" @input="$emit(\'update:value\', $event.target.value)" />'
        },
        'n-button': {
          emits: ['click'],
          template: '<button type="button" @click="$emit(\'click\')"><slot /></button>'
        },
        'n-alert': { props: ['title'], template: '<div>{{ title }}</div>' }
      } }
    });

    await wrapper.get('[data-test="select-repo-directory"]').trigger('click');
    await wrapper.get('[data-test="select-repo-directory"]').trigger('click');
    await flushPromises();
    rejectOld(new Error('旧目录失败'));
    await flushPromises();

    expect((wrapper.findAll('input')[0].element as HTMLInputElement).value).toBe('E:/Projects/new');
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('组件销毁后到达的目录错误不产生通知', async () => {
    let rejectSelection!: (reason: unknown) => void;
    vi.mocked(revierClient.projects.selectDirectory).mockReturnValue(
      new Promise((_, reject) => { rejectSelection = reject; })
    );
    const wrapper = mount(ProjectEditor, { global: { stubs: {
      'n-input': true,
      'n-button': { emits: ['click'], template: '<button @click="$emit(\'click\')"><slot /></button>' },
      'n-alert': true
    } } });

    await wrapper.get('[data-test="select-repo-directory"]').trigger('click');
    wrapper.unmount();
    rejectSelection(new Error('页面已关闭'));
    await flushPromises();

    expect(useNotifications().notifications.value).toEqual([]);
  });
});
