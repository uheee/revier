import {
  attachConsole,
  debug,
  error,
  info,
  trace,
  warn,
  type LogOptions
} from '@tauri-apps/plugin-log';
import { toErrorMessage } from './errors';

type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error';

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
  context?: Record<string, string | number | boolean | undefined>
): Promise<void> {
  if (!isTauriRuntime()) {
    writeConsole(level, message);
    return;
  }
  const keyValues = context
    ? Object.fromEntries(
      Object.entries(context)
        .filter(([, value]) => value !== undefined)
        .map(([key, value]) => [key, String(value)])
    )
    : undefined;
  await writers[level](message, keyValues ? { keyValues } : undefined);
}

export function logError(
  message: string,
  errorValue?: unknown,
  context?: Record<string, string | number | boolean | undefined>
): void {
  const detail = errorValue === undefined ? undefined : toErrorMessage(errorValue);
  const logMessage = detail ? `${message}：${detail}` : message;
  if (!isTauriRuntime()) {
    console.error(message, errorValue);
    return;
  }
  void writeLog('error', logMessage, context);
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
