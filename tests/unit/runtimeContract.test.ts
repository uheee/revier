import {
  contractValidationCode,
  parseBoundary,
  safeIssueSummary
} from '../../src/renderer/contracts/runtime/parseBoundary';
import {
  editorSettingsSnapshotSchema,
  reviewLayoutSizesSchema,
  reviewProjectListSchema
} from '../../src/renderer/contracts/runtime/schemas';

const validProject = {
  id: 'project-1',
  name: 'Revier',
  repoPath: 'E:/Projects/revier',
  pinned: true,
  lastOpenedAt: '2026-07-29T00:00:00.000Z',
  preferences: {
    defaultBranch: 'develop',
    defaultDays: 30,
    defaultGlobRules: ['src/**/*.ts']
  }
};

const validColors = {
  workspaceBackground: '#F4F6F8',
  panelBackground: '#FFFFFF',
  editorBackground: '#FCFDFE',
  border: '#DFE5EC',
  foreground: '#273448',
  muted: '#768296',
  accent: '#0F766E',
  selection: '#DCEFEB',
  diffRemoved: '#FBE7E5',
  diffRemovedStrong: '#BC3D35',
  diffRemovedWord: '#F1B9B3',
  diffAdded: '#E2F3E8',
  diffAddedStrong: '#26804A',
  diffAddedWord: '#A9DBBB',
  syntax: {
    comment: '#768296',
    keyword: '#893CAD',
    string: '#0B7952',
    number: '#A05B00',
    type: '#0969DA',
    function: '#1C63A5',
    variable: '#273448'
  }
};

const validSettingsSnapshot = {
  configPath: 'C:/config/editor.toml',
  settings: {
    version: 1,
    theme: 'system',
    defaultEncoding: 'auto',
    editor: {
      fontFamilies: ['JetBrainsMono Nerd Font Mono', 'Microsoft YaHei', 'monospace'],
      fontSize: 13,
      lineHeight: 22,
      minimap: true
    },
    largeFile: {
      maxBytes: 1_048_576,
      maxLines: 5_000
    },
    themes: {
      light: validColors,
      dark: { ...validColors, workspaceBackground: '#111821' }
    }
  }
};

describe('runtime contract parser', () => {
  it('合法 payload 返回已验证对象并允许额外字段', () => {
    const parsed = parseBoundary(reviewProjectListSchema, [validProject], {
      source: 'projects_list'
    });

    expect(parsed).toEqual([validProject]);
  });

  it('校验失败返回稳定错误码和安全字段路径', () => {
    expect(() =>
      parseBoundary(reviewProjectListSchema, [{ ...validProject, repoPath: 42 }], {
        source: 'projects_list'
      })
    ).toThrow(expect.objectContaining({
      code: contractValidationCode,
      source: 'projects_list',
      issues: expect.arrayContaining([
        expect.objectContaining({ path: '0.repoPath' })
      ])
    }));
  });

  it('错误摘要不包含完整 payload 或敏感字段值', () => {
    const summary = safeIssueSummary(reviewProjectListSchema.safeParse([
      { ...validProject, repoPath: 'E:/secret/repo', preferences: null }
    ]));

    expect(JSON.stringify(summary)).not.toContain('E:/secret/repo');
    expect(JSON.stringify(summary)).not.toContain('Revier');
  });

  it('编辑器设置快照拒绝未知主题和非法数值', () => {
    expect(() =>
      parseBoundary(
        editorSettingsSnapshotSchema,
        {
          ...validSettingsSnapshot,
          settings: {
            ...validSettingsSnapshot.settings,
            theme: 'sepia',
            editor: { ...validSettingsSnapshot.settings.editor, fontSize: -1 }
          }
        },
        { source: 'editor_settings_get' }
      )
    ).toThrow(expect.objectContaining({ code: contractValidationCode }));
  });

  it('layout 持久化边界只接受有限数值', () => {
    expect(parseBoundary(reviewLayoutSizesSchema, { left: 320, right: 360 }, {
      source: 'localStorage:revier.reviewLayout.v1'
    })).toEqual({ left: 320, right: 360 });
    expect(() =>
      parseBoundary(reviewLayoutSizesSchema, { left: Number.NaN, right: 360 }, {
        source: 'localStorage:revier.reviewLayout.v1'
      })
    ).toThrow(expect.objectContaining({ code: contractValidationCode }));
  });
});
