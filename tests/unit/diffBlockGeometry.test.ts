import { describe, expect, it, vi } from 'vitest';
import type { editor } from 'monaco-editor';
import type { AuthorSummary, DiffBlock } from '../../src/renderer/generated/bindings';
import {
  AUTHOR_POPOVER_MAX_HEIGHT,
  AUTHOR_RAIL_WIDTH,
  AUTHOR_ROW_GAP,
  AUTHOR_ROW_HEIGHT,
  fitAuthors,
  getDiffBlockGeometry,
  sortBlockAuthors,
  type DiffBlockGeometryEditor
} from '../../src/renderer/editor/diffBlockGeometry';

const authors: AuthorSummary[] = [
  { name: '低频', email: 'low@example.com', commitCount: 1, lastCommittedAt: '2026-01-03T00:00:00Z' },
  { name: '高频但较旧', email: 'old@example.com', commitCount: 3, lastCommittedAt: '2026-01-01T00:00:00Z' },
  { name: '高频且较新', email: 'new@example.com', commitCount: 3, lastCommittedAt: '2026-01-02T00:00:00Z' }
];
const LINE_HEIGHT_OPTION = 67 as editor.EditorOption.lineHeight;

function block(changeType: DiffBlock['changeType'] = 'modified'): DiffBlock {
  return {
    id: 'block-1',
    oldStart: 10,
    oldEnd: 13,
    newStart: 20,
    newEnd: 23,
    changeType,
    authors: [],
    rows: [],
    relatedCommits: []
  };
}

function geometryEditor(options: {
  visibleRanges?: Array<{ startLineNumber: number; endLineNumber: number }>;
  scrollTop?: number;
  lineHeight?: number;
  layoutHeight?: number;
  nullPositions?: number[];
  topForLine?: (lineNumber: number) => number;
} = {}): DiffBlockGeometryEditor {
  const scrollTop = options.scrollTop ?? 0;
  const lineHeight = options.lineHeight ?? 20;
  const nullPositions = new Set(options.nullPositions ?? []);
  const topForLine = options.topForLine ?? ((lineNumber: number) => (lineNumber - 1) * lineHeight);
  return {
    getVisibleRanges: vi.fn(() => options.visibleRanges ?? [{ startLineNumber: 1, endLineNumber: 100 }]),
    getTopForLineNumber: vi.fn(topForLine),
    getScrolledVisiblePosition: vi.fn((position: { lineNumber: number }) => (
      nullPositions.has(position.lineNumber)
        ? null
        : { top: topForLine(position.lineNumber) - scrollTop, left: 0, height: lineHeight }
    )),
    getScrollTop: vi.fn(() => scrollTop),
    getOption: vi.fn(() => lineHeight),
    getLayoutInfo: vi.fn(() => ({ height: options.layoutHeight ?? 200 }))
  };
}

describe('diffBlockGeometry', () => {
  it('锁定轨道布局常量', () => {
    expect({ AUTHOR_RAIL_WIDTH, AUTHOR_ROW_HEIGHT, AUTHOR_ROW_GAP, AUTHOR_POPOVER_MAX_HEIGHT })
      .toEqual({ AUTHOR_RAIL_WIDTH: 112, AUTHOR_ROW_HEIGHT: 20, AUTHOR_ROW_GAP: 2, AUTHOR_POPOVER_MAX_HEIGHT: 320 });
  });

  it('按提交次数、合法提交时间和中文姓名排序且不修改输入', () => {
    const input = [
      ...authors,
      { name: '乙', commitCount: 3, lastCommittedAt: '无效时间' },
      { name: '甲', commitCount: 3, lastCommittedAt: '' }
    ];
    const snapshot = [...input];

    expect(sortBlockAuthors(input).map((author) => author.name))
      .toEqual(['高频且较新', '高频但较旧', '甲', '乙', '低频']);
    expect(input).toEqual(snapshot);
  });

  it('按调用方给定顺序裁剪作者并为隐藏项保留省略号槽位', () => {
    const sorted = sortBlockAuthors([
      ...authors,
      { name: '更低频', commitCount: 0, lastCommittedAt: '' }
    ]);
    expect(fitAuthors(20, sorted)).toEqual({ visible: [], hasMore: true });
    expect(fitAuthors(42, sorted)).toEqual({ visible: [sorted[0]], hasMore: true });
    expect(fitAuthors(64, sorted)).toEqual({ visible: sorted.slice(0, 2), hasMore: true });
    expect(fitAuthors(42, sorted.slice(0, 2))).toEqual({ visible: sorted.slice(0, 2), hasMore: false });
    expect(fitAuthors(0, [])).toEqual({ visible: [], hasMore: false });
  });

  it('修改和新增块使用修改侧，删除块使用原始侧', () => {
    const original = geometryEditor({ scrollTop: 180 });
    const modified = geometryEditor({ scrollTop: 380 });

    expect(getDiffBlockGeometry(block('modified'), original, modified, LINE_HEIGHT_OPTION)).toEqual({ top: 0, height: 80 });
    expect(getDiffBlockGeometry(block('added'), original, modified, LINE_HEIGHT_OPTION)).toEqual({ top: 0, height: 80 });
    expect(getDiffBlockGeometry(block('deleted'), original, modified, LINE_HEIGHT_OPTION)).toEqual({ top: 0, height: 80 });
    expect(modified.getTopForLineNumber).toHaveBeenCalledWith(20);
    expect(original.getTopForLineNumber).toHaveBeenCalledWith(10);
  });

  it('把部分出视口的块裁剪到视口且高度不超过可见块高度', () => {
    const original = geometryEditor();
    const modified = geometryEditor({ scrollTop: 410, layoutHeight: 60 });

    expect(getDiffBlockGeometry(block(), original, modified, LINE_HEIGHT_OPTION)).toEqual({ top: 0, height: 50 });
  });

  it('完全出视口、零行范围或完全折叠时隐藏', () => {
    const editor = geometryEditor({ visibleRanges: [{ startLineNumber: 1, endLineNumber: 8 }] });
    expect(getDiffBlockGeometry(block(), editor, editor, LINE_HEIGHT_OPTION)).toBeUndefined();

    const zeroRange = { ...block('added'), newStart: 0, newEnd: 0 };
    expect(getDiffBlockGeometry(zeroRange, editor, editor, LINE_HEIGHT_OPTION)).toBeUndefined();

    const folded = geometryEditor({ visibleRanges: [{ startLineNumber: 1, endLineNumber: 19 }, { startLineNumber: 24, endLineNumber: 30 }] });
    expect(getDiffBlockGeometry(block(), folded, folded, LINE_HEIGHT_OPTION)).toBeUndefined();
  });

  it('跨折叠区时只使用公开可见行坐标，并为边界空位置回退到公开滚动坐标', () => {
    const editor = geometryEditor({
      visibleRanges: [{ startLineNumber: 20, endLineNumber: 20 }, { startLineNumber: 23, endLineNumber: 30 }],
      scrollTop: 380,
      layoutHeight: 100,
      nullPositions: [20, 23],
      topForLine: (lineNumber) => ({ 20: 380, 23: 400 }[lineNumber] ?? 0)
    });

    expect(getDiffBlockGeometry(block(), editor, editor, LINE_HEIGHT_OPTION)).toEqual({ top: 0, height: 40 });
    expect(editor.getScrollTop).toHaveBeenCalled();
  });
});
