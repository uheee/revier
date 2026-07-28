import type { z, ZodType } from 'zod';

export const contractValidationCode = 'CONTRACT_VALIDATION_FAILED';

export interface BoundaryParseOptions {
  source: string;
}

export interface ContractIssueSummary {
  path: string;
  code: string;
  message: string;
}

class ContractValidationError extends Error {
  readonly code = contractValidationCode;
  readonly source: string;
  readonly issues: ContractIssueSummary[];

  constructor(source: string, issues: ContractIssueSummary[]) {
    super('应用数据格式不兼容，请重试或升级应用');
    this.name = 'ContractValidationError';
    this.source = source;
    this.issues = issues;
  }
}

export function parseBoundary<T>(
  schema: ZodType<T>,
  payload: unknown,
  options: BoundaryParseOptions
): T {
  const result = schema.safeParse(payload);
  if (result.success) {
    return result.data;
  }
  throw new ContractValidationError(options.source, safeIssueSummary(result));
}

export function safeIssueSummary(result: z.ZodSafeParseResult<unknown>): ContractIssueSummary[] {
  if (result.success) {
    return [];
  }
  return result.error.issues.map((issue) => ({
    path: issue.path.map(String).join('.') || '<root>',
    code: issue.code,
    message: issue.message
  }));
}
