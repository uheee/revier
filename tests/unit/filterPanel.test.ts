// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import { defineComponent } from 'vue';
import FilterPanel from '../../src/renderer/components/review/FilterPanel.vue';

describe('FilterPanel', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-06-12T12:00:00.000Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('emits structured filters from branch, authors and date range controls', async () => {
    const wrapper = mount(FilterPanel, {
      props: {
        projectId: 'project-1',
        defaultBranch: 'develop',
        defaultDays: 30,
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

  it('打开时在时间范围中显示默认 30 天', () => {
    const wrapper = mount(FilterPanel, {
      props: {
        projectId: 'project-1',
        defaultBranch: 'develop',
        defaultDays: 30,
        defaultGlobRules: [],
        branches: [{ name: 'develop', current: true }],
        authors: [],
        loading: false
      },
      global: {
        stubs: filterPanelStubs()
      }
    });

    expect(wrapper.findComponent(DatePickerStub).props('value')).toEqual([
      Date.parse('2026-05-13T12:00:00.000Z'),
      Date.parse('2026-06-12T12:00:00.000Z')
    ]);
  });

  it('打开时恢复已保存的筛选条件', async () => {
    const wrapper = mount(FilterPanel, {
      props: {
        projectId: 'project-1',
        defaultBranch: 'develop',
        defaultDays: 30,
        defaultGlobRules: ['src/**/*.ts'],
        savedFilters: {
          branch: 'main',
          startAt: '2026-05-01T00:00:00.000Z',
          endAt: '2026-06-01T00:00:00.000Z',
          authorKeys: ['alice@example.com'],
          messageQuery: 'feature',
          globRules: ['docs/**/*.md']
        },
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
        stubs: filterPanelStubs()
      }
    });

    await wrapper.find('form').trigger('submit.prevent');

    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      projectId: 'project-1',
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-06-01T00:00:00.000Z',
      authorKeys: ['alice@example.com'],
      messageQuery: 'feature',
      globRules: ['docs/**/*.md']
    });
  });

  it('筛选条件变化时发出 change 事件', async () => {
    const wrapper = mount(FilterPanel, {
      props: {
        projectId: 'project-1',
        defaultBranch: 'develop',
        defaultDays: 30,
        defaultGlobRules: [],
        branches: [
          { name: 'develop', current: true },
          { name: 'main', current: false }
        ],
        authors: [],
        loading: false
      },
      global: {
        stubs: filterPanelStubs()
      }
    });

    await wrapper.get('[data-test="set-branch"]').trigger('click');

    expect(wrapper.emitted('change')?.at(-1)?.[0]).toMatchObject({
      projectId: 'project-1',
      branch: 'main'
    });
  });

  it('按分支缓存状态显示分析或重新分析项目', async () => {
    const wrapper = mount(FilterPanel, {
      props: {
        projectId: 'project-1',
        defaultBranch: 'develop',
        defaultGlobRules: [],
        branches: [{ name: 'develop', current: true }],
        authors: [],
        cacheState: 'miss'
      },
      global: { stubs: filterPanelStubs() }
    });

    expect(wrapper.text()).toContain('分析项目');
    expect(wrapper.text()).not.toContain('缓存可能已过期');

    await wrapper.setProps({ cacheState: 'stale' });
    expect(wrapper.text()).toContain('重新分析项目');
    expect(wrapper.text()).toContain('缓存可能已过期');
  });
});

const SelectStub = defineComponent({
  name: 'NSelect',
  props: ['value', 'multiple'],
  emits: ['update:value'],
  template:
    '<div><button type="button" data-test="set-branch" @click="$emit(\'update:value\', multiple !== undefined ? [\'alice@example.com\'] : \'main\')">set</button><slot /></div>'
});

const DatePickerStub = defineComponent({
  name: 'NDatePicker',
  props: ['value'],
  emits: ['update:value'],
  template:
    '<div><button type="button" data-test="set-date-range" @click="$emit(\'update:value\', [Date.parse(\'2026-05-01T00:00:00.000Z\'), Date.parse(\'2026-06-01T00:00:00.000Z\')])">date</button></div>'
});

function filterPanelStubs() {
  return {
    'n-select': SelectStub,
    'n-date-picker': DatePickerStub,
    'n-input': {
      props: ['value'],
      emits: ['update:value'],
      template: '<textarea :value="value" @input="$emit(\'update:value\', $event.target.value)" />'
    },
    'n-button': {
      template: '<button type="submit"><slot /></button>'
    }
  };
}
