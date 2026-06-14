export interface AnalyzeRequest { repoPath: string; baseCommit: string; headCommit: string; globRules: string[]; format?: 'json'; pretty?: boolean; explain?: boolean; }
export interface FileOverlayAnalysisRequest { repoPath: string; baseCommit: string; headCommit: string; filePath: string; oldPath?: string; format?: 'json'; pretty?: boolean; explain?: boolean; }
export interface TraceBlockRequest extends FileOverlayAnalysisRequest { blockId: string; }
