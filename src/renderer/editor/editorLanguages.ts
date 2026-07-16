export interface EditorLanguageDefinition {
  readonly id: string;
  readonly label: string;
  readonly extensions: readonly string[];
  readonly filenames?: readonly string[];
  readonly shikiLanguage?: string;
}

function freezeLanguage(
  language: EditorLanguageDefinition
): EditorLanguageDefinition {
  return Object.freeze({
    ...language,
    extensions: Object.freeze([...language.extensions]),
    filenames: language.filenames
      ? Object.freeze([...language.filenames])
      : undefined
  });
}

export const EDITOR_LANGUAGES: readonly EditorLanguageDefinition[] = Object.freeze([
  { id: 'plaintext', label: '纯文本', extensions: [] },
  { id: 'javascript', label: 'JavaScript', extensions: ['.js', '.jsx', '.mjs', '.cjs'], shikiLanguage: 'javascript' },
  { id: 'typescript', label: 'TypeScript', extensions: ['.d.ts', '.ts', '.tsx', '.mts', '.cts'], shikiLanguage: 'typescript' },
  { id: 'vue', label: 'Vue', extensions: ['.vue'], shikiLanguage: 'vue' },
  { id: 'html', label: 'HTML', extensions: ['.html', '.htm'], shikiLanguage: 'html' },
  { id: 'css', label: 'CSS', extensions: ['.css', '.scss', '.sass', '.less'], shikiLanguage: 'css' },
  { id: 'rust', label: 'Rust', extensions: ['.rs'], shikiLanguage: 'rust' },
  { id: 'python', label: 'Python', extensions: ['.py', '.pyi'], shikiLanguage: 'python' },
  { id: 'go', label: 'Go', extensions: ['.go'], shikiLanguage: 'go' },
  { id: 'java', label: 'Java', extensions: ['.java'], shikiLanguage: 'java' },
  { id: 'c', label: 'C', extensions: ['.c', '.h'], shikiLanguage: 'c' },
  { id: 'cpp', label: 'C++', extensions: ['.cpp', '.cc', '.cxx', '.hpp', '.hh', '.hxx'], shikiLanguage: 'cpp' },
  { id: 'csharp', label: 'C#', extensions: ['.cs'], shikiLanguage: 'csharp' },
  { id: 'sql', label: 'SQL', extensions: ['.sql'], shikiLanguage: 'sql' },
  { id: 'markdown', label: 'Markdown', extensions: ['.md', '.markdown'], shikiLanguage: 'markdown' },
  { id: 'json', label: 'JSON', extensions: ['.json', '.jsonc'], shikiLanguage: 'json' },
  { id: 'yaml', label: 'YAML', extensions: ['.yaml', '.yml'], shikiLanguage: 'yaml' },
  { id: 'toml', label: 'TOML', extensions: ['.toml'], shikiLanguage: 'toml' },
  { id: 'shell', label: 'Shell', extensions: ['.sh', '.bash'], filenames: ['Makefile'], shikiLanguage: 'shellscript' },
  { id: 'powershell', label: 'PowerShell', extensions: ['.ps1', '.psm1'], shikiLanguage: 'powershell' }
].map(freezeLanguage));

const filenameLanguages = new Map(
  EDITOR_LANGUAGES.flatMap((language) =>
    (language.filenames ?? []).map((filename) => [filename.toLowerCase(), language.id] as const)
  )
);

const extensionLanguages = EDITOR_LANGUAGES.flatMap((language) =>
  language.extensions.map((extension) => ({ extension: extension.toLowerCase(), id: language.id }))
).sort((left, right) => right.extension.length - left.extension.length);

export function detectEditorLanguage(path: string): string {
  const filename = path.replaceAll('\\', '/').split('/').at(-1)?.toLowerCase() ?? '';
  const filenameMatch = filenameLanguages.get(filename);
  if (filenameMatch) {
    return filenameMatch;
  }
  return extensionLanguages.find(({ extension }) => filename.endsWith(extension))?.id ?? 'plaintext';
}
