import { minimatch } from 'minimatch';
import { normalizeGitPath } from './changedFiles';

export function createGlobMatcher(rules: string[]): (path: string) => boolean {
  const include = rules.filter((rule) => !rule.startsWith('!'));
  const exclude = rules.filter((rule) => rule.startsWith('!')).map((rule) => rule.slice(1));

  return (path: string) => {
    const normalized = normalizeGitPath(path);
    const included =
      include.length === 0 || include.some((rule) => minimatch(normalized, rule, { dot: true }));
    const excluded = exclude.some((rule) => minimatch(normalized, rule, { dot: true }));
    return included && !excluded;
  };
}
