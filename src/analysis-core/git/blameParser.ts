import type { BlameLine } from './analysisGitClient';

export function parseBlamePorcelain(output: string): BlameLine[] {
  const lines = output.split('\n');
  const result: BlameLine[] = [];
  let current: Partial<BlameLine> | undefined;
  for (const line of lines) {
    const header = line.match(/^([0-9a-f]{40})\s+(\d+)\s+(\d+)/i);
    if (header) {
      current = { commitHash: header[1], originalLineNumber: Number(header[2]), lineNumber: Number(header[3]), committedAt: '', subject: '', authorName: '' };
      continue;
    }
    if (!current) continue;
    if (line.startsWith('author ')) current.authorName = line.slice(7);
    else if (line.startsWith('author-mail ')) current.authorEmail = line.slice(12).replace(/^<|>$/g, '');
    else if (line.startsWith('author-time ')) current.committedAt = new Date(Number(line.slice(12)) * 1000).toISOString();
    else if (line.startsWith('summary ')) current.subject = line.slice(8);
    else if (line.startsWith('\t')) {
      result.push({ ...(current as Omit<BlameLine, 'content'>), content: line.slice(1) });
      current = undefined;
    }
  }
  return result;
}
