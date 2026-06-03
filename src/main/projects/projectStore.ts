import { randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { basename, dirname } from 'node:path';
import type { ProjectPreferences, ReviewProject } from '../../shared/projectTypes';

interface ProjectStoreFile {
  projects: ReviewProject[];
}

const defaultPreferences: ProjectPreferences = {
  defaultDays: 30,
  defaultGlobRules: []
};

export class JsonProjectStore {
  constructor(private readonly filePath: string) {}

  async list(): Promise<ReviewProject[]> {
    return (await this.read()).projects;
  }

  async add(repoPath: string, options: Partial<ReviewProject> = {}): Promise<ReviewProject> {
    const data = await this.read();
    const project: ReviewProject = {
      id: randomUUID(),
      name: options.name?.trim() || basename(repoPath),
      repoPath,
      pinned: options.pinned ?? false,
      lastOpenedAt: new Date().toISOString(),
      preferences: {
        ...defaultPreferences,
        ...options.preferences,
        defaultGlobRules: options.preferences?.defaultGlobRules ?? defaultPreferences.defaultGlobRules
      }
    };

    data.projects.push(project);
    await this.write(data);
    return project;
  }

  async update(project: ReviewProject): Promise<ReviewProject> {
    const data = await this.read();
    const index = data.projects.findIndex((item) => item.id === project.id);
    if (index === -1) {
      throw new Error(`Project not found: ${project.id}`);
    }

    data.projects[index] = project;
    await this.write(data);
    return project;
  }

  async remove(projectId: string): Promise<void> {
    const data = await this.read();
    data.projects = data.projects.filter((project) => project.id !== projectId);
    await this.write(data);
  }

  private async read(): Promise<ProjectStoreFile> {
    try {
      return JSON.parse(await readFile(this.filePath, 'utf8')) as ProjectStoreFile;
    } catch (error) {
      if (isMissingFileError(error)) {
        return { projects: [] };
      }
      throw error;
    }
  }

  private async write(data: ProjectStoreFile): Promise<void> {
    await mkdir(dirname(this.filePath), { recursive: true });
    await writeFile(this.filePath, `${JSON.stringify(data, null, 2)}\n`, 'utf8');
  }
}

function isMissingFileError(error: unknown): boolean {
  return typeof error === 'object' && error !== null && 'code' in error && error.code === 'ENOENT';
}
