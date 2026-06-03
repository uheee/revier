export type AppErrorCode =
  | 'NOT_A_GIT_REPOSITORY'
  | 'REPOSITORY_NOT_FOUND'
  | 'GIT_NOT_INSTALLED'
  | 'BRANCH_NOT_FOUND'
  | 'GIT_COMMAND_TIMEOUT'
  | 'FILE_TOO_LARGE'
  | 'BINARY_FILE'
  | 'PATCH_PARSE_FAILED'
  | 'UNKNOWN';

export interface AppError {
  code: AppErrorCode;
  message: string;
  recoverable: boolean;
  detail?: string;
}

export function createAppError(
  code: AppErrorCode,
  message: string,
  recoverable: boolean,
  detail?: string
): AppError {
  return { code, message, recoverable, detail };
}

export function isRecoverableError(error: AppError): boolean {
  return error.recoverable;
}
