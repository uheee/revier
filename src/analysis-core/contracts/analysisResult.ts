import type { AppError } from '../../shared/errors';
import type { ChangedFile, FileOverlay } from '../../shared/reviewTypes';
export interface AnalysisCliResult { version: 1; range: { baseCommit: string; headCommit: string }; files: ChangedFile[]; overlays: FileOverlay[]; warnings: AppError[]; }
export interface TraceBlockResult { version: 1; overlay: FileOverlay; blockId: string; }
