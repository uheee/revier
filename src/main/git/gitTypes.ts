export interface GitCommitSummary {
  hash: string;
  shortHash: string;
  authorName: string;
  authorEmail?: string;
  committedAt: string;
  subject: string;
}
