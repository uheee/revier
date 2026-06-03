export type ProjectId = string;

export interface ProjectPreferences {
  defaultBranch?: string;
  defaultDays?: number;
  defaultGlobRules: string[];
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
