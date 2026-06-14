import { parseArgs } from '../../src/cli/revier-analysis';

describe('revier analysis CLI args', () => {
  it('parses clap-style analyze arguments', () => {
    expect(parseArgs(['analyze', '--repo', '/repo', '--base', 'base', '--head', 'head', '--glob', 'src/**/*.ts', '--pretty'])).toEqual({
      command: 'analyze',
      repo: '/repo',
      base: 'base',
      head: 'head',
      glob: ['src/**/*.ts'],
      format: 'json',
      pretty: true
    });
  });

  it('rejects non-json formats', () => {
    expect(() => parseArgs(['analyze', '--format', 'text'])).toThrow('Only --format json is supported');
  });
});
