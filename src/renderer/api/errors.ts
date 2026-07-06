export function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (hasMessage(error)) {
    return error.message;
  }

  if (hasCodeOrDetail(error)) {
    return [error.code, error.detail].filter(Boolean).join(': ');
  }

  if (typeof error === 'object' && error !== null) {
    try {
      return JSON.stringify(error);
    } catch {
      return '未知错误';
    }
  }

  return String(error);
}

function hasMessage(error: unknown): error is { message: string } {
  return (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof (error as { message?: unknown }).message === 'string'
  );
}

function hasCodeOrDetail(error: unknown): error is { code?: string; detail?: string } {
  return (
    typeof error === 'object' &&
    error !== null &&
    (typeof (error as { code?: unknown }).code === 'string' ||
      typeof (error as { detail?: unknown }).detail === 'string')
  );
}
