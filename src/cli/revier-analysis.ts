#!/usr/bin/env node
import { buildFileOverlay, SimpleGitAnalysisClient } from '../analysis-core';
import type { AnalysisCliResult } from '../analysis-core';

interface ParsedArgs { command?: string; repo?: string; base?: string; head?: string; file?: string; glob: string[]; format: 'json'; pretty: boolean; }

export function parseArgs(argv: string[]): ParsedArgs {
  const args: ParsedArgs = { command: argv[0], glob: [], format: 'json', pretty: false };
  for (let i = 1; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === '--pretty') args.pretty = true;
    else if (token === '--repo') args.repo = argv[++i];
    else if (token === '--base') args.base = argv[++i];
    else if (token === '--head') args.head = argv[++i];
    else if (token === '--file') args.file = argv[++i];
    else if (token === '--glob') args.glob.push(argv[++i]);
    else if (token === '--format') { const format = argv[++i]; if (format !== 'json') throw new Error('Only --format json is supported'); }
    else throw new Error(`Unknown argument: ${token}`);
  }
  return args;
}

export async function run(argv = process.argv.slice(2)): Promise<number> {
  try {
    const args = parseArgs(argv);
    if (!args.command || !['analyze', 'file-overlay'].includes(args.command)) throw new Error('Expected subcommand: analyze or file-overlay');
    if (!args.repo || !args.base || !args.head) throw new Error('Missing required --repo, --base, or --head');
    const git = new SimpleGitAnalysisClient();
    const files = await git.listChangedFiles(args.repo, args.base, args.head);
    const selected = args.command === 'file-overlay' ? files.filter((file) => file.path === args.file) : files;
    if (args.command === 'file-overlay' && !args.file) throw new Error('Missing required --file');
    const overlays = [];
    for (const file of selected) {
      overlays.push(await buildFileOverlay({ repoPath: args.repo, file, range: { branch: '', baseCommit: args.base, headCommit: args.head }, rangeCommits: [], filters: { projectId: 'cli', branch: '', globRules: args.glob }, git }));
    }
    const result: AnalysisCliResult = { version: 1, range: { baseCommit: args.base, headCommit: args.head }, files: selected, overlays, warnings: [] };
    process.stdout.write(`${JSON.stringify(result, null, args.pretty ? 2 : 0)}\n`);
    return 0;
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.exitCode = await run();
}
