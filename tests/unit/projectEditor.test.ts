// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils';
import ProjectEditor from '../../src/renderer/components/projects/ProjectEditor.vue';
import { revierClient } from '../../src/renderer/api/revierClient';

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
  });
});
