// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import ProjectEditor from '../../src/renderer/components/projects/ProjectEditor.vue';

describe('ProjectEditor', () => {
  afterEach(() => {
    delete (window as unknown as { revier?: unknown }).revier;
  });

  it('selects a directory and fills repo path and project name', async () => {
    const selectDirectory = vi.fn(async () => ({
      path: 'E:/Projects/revier',
      name: 'revier'
    }));
    (window as unknown as { revier: unknown }).revier = {
      projects: { selectDirectory }
    };

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
          }
        }
      }
    });

    await wrapper.get('[data-test="select-repo-directory"]').trigger('click');
    const inputs = wrapper.findAll('input');

    expect(selectDirectory).toHaveBeenCalledTimes(1);
    expect((inputs[0].element as HTMLInputElement).value).toBe('E:/Projects/revier');
    expect((inputs[1].element as HTMLInputElement).value).toBe('revier');
  });
});
