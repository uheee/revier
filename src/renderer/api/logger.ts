import {
  attachConsole,
  debug,
  error,
  info,
  trace,
  warn,
  type LogOptions
} from '@tauri-apps/plugin-log';

type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error';
type LogContextValue = string | number | boolean | undefined;
type SafeLogContextKey =
  | 'source'
  | 'taskId'
  | 'operationId'
  | 'stage'
  | 'status'
  | 'elapsedMs'
  | 'count'
  | 'fileCount'
  | 'cacheState'
  | 'queryType';
type LogContext = Partial<Record<SafeLogContextKey, LogContextValue>>;

const SAFE_CONTEXT_KEYS = new Set<SafeLogContextKey>([
  'source',
  'taskId',
  'operationId',
  'stage',
  'status',
  'elapsedMs',
  'count',
  'fileCount',
  'cacheState',
  'queryType'
]);

const EMAIL_PATTERN = /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi;
const EDGE_PUNCTUATION_PATTERN = /^[,.;:!?，。；：！？()[\]{}<>"']+|[,.;:!?，。；：！？()[\]{}<>"']+$/g;
const OMITTED_ERROR_DETAIL = '错误详情已省略';
const SAFE_ERROR_CODE_PATTERN = /^[A-Z][A-Z0-9_-]{0,63}$/i;

const writers: Record<LogLevel, (message: string, options?: LogOptions) => Promise<void>> = {
  trace,
  debug,
  info,
  warn,
  error
};

let detachConsole: (() => void) | undefined;

export async function initializeRendererLogger(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }
  if (detachConsole) {
    return;
  }
  detachConsole = await attachConsole();
}

async function writeLog(
  level: LogLevel,
  message: string,
  context?: LogContext
): Promise<void> {
  const safeMessage = redactSensitiveLogText(message);
  if (!isTauriRuntime()) {
    writeConsole(level, safeMessage);
    return;
  }
  const keyValues = context
    ? Object.fromEntries(
      Object.entries(context)
        .filter(([key, value]) =>
          SAFE_CONTEXT_KEYS.has(key as SafeLogContextKey) && value !== undefined
        )
        .map(([key, value]) => [key, redactSensitiveLogText(String(value))])
    )
    : undefined;
  await writers[level](safeMessage, keyValues ? { keyValues } : undefined);
}

export function logError(
  message: string,
  errorValue?: unknown,
  context?: LogContext
): void {
  const detail = errorValue === undefined ? undefined : toSafeErrorMessage(errorValue);
  const logMessage = detail ? `${message}：${detail}` : message;
  void writeLog('error', logMessage, context);
}

function toSafeErrorMessage(errorValue: unknown): string {
  if (errorValue instanceof AggregateError) {
    return 'AggregateError';
  }
  if (errorValue instanceof Error) {
    return 'Error';
  }
  if (typeof errorValue === 'object' && errorValue !== null) {
    const code = (errorValue as { code?: unknown }).code;
    if (typeof code === 'string' && SAFE_ERROR_CODE_PATTERN.test(code)) {
      return `错误码 ${code}`;
    }
  }
  return OMITTED_ERROR_DETAIL;
}

export function redactSensitiveLogText(message: string): string {
  return message
    .replace(EMAIL_PATTERN, '[邮箱已脱敏]')
    .split(/(\s+)/)
    .map((segment) => {
      const content = segment.replace(EDGE_PUNCTUATION_PATTERN, '');
      return content.length > 1 && (content.includes('/') || content.includes('\\'))
        ? '[路径已脱敏]'
        : segment;
    })
    .join('');
}

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function writeConsole(level: LogLevel, message: string): void {
  if (level === 'error') {
    console.error(message);
    return;
  }
  if (level === 'warn') {
    console.warn(message);
    return;
  }
  if (level === 'debug' || level === 'trace') {
    console.debug(message);
    return;
  }
  console.info(message);
}
