import type { ChangedFile } from '../../shared/reviewTypes';

export function normalizeGitPath(path: string): string {
  return path.replaceAll('\\', '/');
}

export function parseNameStatus(output: string): ChangedFile[] {
  return output
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => {
      const [statusToken, firstPath, secondPath] = line.split('\t');
      const statusCode = statusToken[0];
      const renamed = statusCode === 'R';
      const deleted = statusCode === 'D';
      const path = normalizeGitPath(renamed ? secondPath : firstPath);

      return {
        path,
        oldPath: renamed ? normalizeGitPath(firstPath) : undefined,
        status: renamed ? 'renamed' : statusCode === 'A' ? 'added' : deleted ? 'deleted' : 'modified',
        additions: 0,
        deletions: 0,
        isBinary: false,
        isPreviewable: true
      };
    });
}

export function parseNumstat(files: ChangedFile[], output: string): ChangedFile[] {
  const stats = new Map<string, { additions: number; deletions: number; binary: boolean }>();
  for (const line of output.split(/\r?\n/).filter(Boolean)) {
    const [adds, dels, path] = line.split('\t');
    stats.set(normalizeGitPath(path), {
      additions: adds === '-' ? 0 : Number(adds),
      deletions: dels === '-' ? 0 : Number(dels),
      binary: adds === '-' || dels === '-'
    });
  }

  const existing = files.map((file) => {
    const stat = stats.get(file.path);
    if (!stat) {
      return file;
    }

    return {
      ...file,
      additions: stat.additions,
      deletions: stat.deletions,
      isBinary: stat.binary,
      isPreviewable: !stat.binary,
      status: stat.binary ? 'binary' : file.status
    };
  });

  for (const [path, stat] of stats) {
    if (!existing.some((file) => file.path === path)) {
      existing.push({
        path,
        status: stat.binary ? 'binary' : 'modified',
        additions: stat.additions,
        deletions: stat.deletions,
        isBinary: stat.binary,
        isPreviewable: !stat.binary
      });
    }
  }

  return existing;
}
