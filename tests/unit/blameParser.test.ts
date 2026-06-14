import { parseBlamePorcelain } from '../../src/analysis-core';

describe('blame porcelain parser', () => {
  it('parses author metadata and blamed content lines', () => {
    const hash = 'a'.repeat(40);
    const lines = parseBlamePorcelain(`${hash} 3 7 1\nauthor Alice\nauthor-mail <alice@example.com>\nauthor-time 1780272000\nsummary feat: add value\n\texport const value = 1;\n`);

    expect(lines).toEqual([
      expect.objectContaining({
        commitHash: hash,
        originalLineNumber: 3,
        lineNumber: 7,
        authorName: 'Alice',
        authorEmail: 'alice@example.com',
        subject: 'feat: add value',
        content: 'export const value = 1;'
      })
    ]);
  });
});
