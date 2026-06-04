// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import FilterPanel from '../../src/renderer/components/review/FilterPanel.vue';

describe('FilterPanel', () => {
  it('emits structured filters from branch, authors and date range controls', async () => {
    const wrapper = mount(FilterPanel, {
      props: {
        projectId: 'project-1',
        defaultBranch: 'develop',
        defaultGlobRules: ['src/**/*.ts'],
        branches: [
          { name: 'develop', current: true },
          { name: 'main', current: false }
        ],
        authors: [
          { key: 'alice@example.com', name: 'Alice', email: 'alice@example.com', commitCount: 2 }
        ],
        loading: false
      },
      global: {
        stubs: {
          'n-select': {
            props: ['value', 'multiple'],
            emits: ['update:value'],
            template:
              '<div><button type="button" data-test="set-select" @click="$emit(\'update:value\', multiple !== undefined ? [\'alice@example.com\'] : \'main\')">set</button><slot /></div>'
          },
          'n-date-picker': {
            emits: ['update:value'],
            template:
              '<div><button type="button" data-test="set-date-range" @click="$emit(\'update:value\', [Date.parse(\'2026-05-01T00:00:00.000Z\'), Date.parse(\'2026-06-01T00:00:00.000Z\')])">date</button></div>'
          },
          'n-input': {
            props: ['value'],
            emits: ['update:value'],
            template: '<textarea :value="value" @input="$emit(\'update:value\', $event.target.value)" />'
          },
          'n-button': {
            template: '<button type="submit"><slot /></button>'
          }
        }
      }
    });

    const selectButtons = wrapper.findAll('[data-test="set-select"]');
    await selectButtons[0].trigger('click');
    await selectButtons[1].trigger('click');
    await wrapper.get('[data-test="set-date-range"]').trigger('click');
    await wrapper.find('form').trigger('submit.prevent');

    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      projectId: 'project-1',
      branch: 'main',
      authorKeys: ['alice@example.com'],
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-06-01T00:00:00.000Z',
      globRules: ['src/**/*.ts']
    });
  });
});
