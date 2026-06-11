export type ProjectId = string;

export interface ProjectReviewFilters {
  branch?: string;
  startAt?: string;
  endAt?: string;
  authorKeys?: string[];
  authorQuery?: string;
  messageQuery?: string;
  globRules?: string[];
}

export interface ProjectPreferences {
  defaultBranch?: string;
  defaultDays?: number;
  defaultGlobRules: string[];
  reviewFilters?: ProjectReviewFilters;
}

export interface ReviewProject {
  id: ProjectId;
  name: string;
  repoPath: string;
  pinned: boolean;
  lastOpenedAt?: string;
  preferences: ProjectPreferences;
}

export interface RepositoryValidation {
  valid: boolean;
  repoPath: string;
  currentBranch?: string;
  error?: string;
}

export interface GitBranch {
  name: string;
  current: boolean;
}

export interface DirectorySelection {
  path: string;
  name: string;
}
