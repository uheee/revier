import { z } from 'zod';
import type {
  EditorSettingsSnapshot,
  ReviewProject
} from '../../generated/bindings';

const isoTextSchema = z.string().min(1);
const nonEmptyTextSchema = z.string().min(1);
const finiteNumberSchema = z.number().finite();
const positiveIntegerSchema = z.number().int().positive();

const textEncodingSchema = z.enum(['auto', 'utf-8', 'gb18030', 'utf-16le', 'utf-16be']);
const editorThemeModeSchema = z.enum(['system', 'light', 'dark']);

const projectReviewFiltersSchema = z.object({
  branch: nonEmptyTextSchema.optional(),
  startAt: isoTextSchema.optional(),
  endAt: isoTextSchema.optional(),
  authorKeys: z.array(nonEmptyTextSchema).optional(),
  messageQuery: z.string().optional(),
  globRules: z.array(nonEmptyTextSchema).optional()
}).passthrough();

const projectPreferencesSchema = z.object({
  defaultBranch: nonEmptyTextSchema.optional(),
  defaultDays: positiveIntegerSchema.optional(),
  defaultGlobRules: z.array(nonEmptyTextSchema),
  reviewFilters: projectReviewFiltersSchema.optional()
}).passthrough();

export const reviewProjectSchema: z.ZodType<ReviewProject> = z.object({
  id: nonEmptyTextSchema,
  name: nonEmptyTextSchema,
  repoPath: nonEmptyTextSchema,
  pinned: z.boolean(),
  lastOpenedAt: isoTextSchema.optional(),
  preferences: projectPreferencesSchema
}).passthrough() as z.ZodType<ReviewProject>;

export const reviewProjectListSchema: z.ZodType<ReviewProject[]> = z.array(reviewProjectSchema);

const editorSyntaxColorsSchema = z.object({
  comment: nonEmptyTextSchema,
  keyword: nonEmptyTextSchema,
  string: nonEmptyTextSchema,
  number: nonEmptyTextSchema,
  type: nonEmptyTextSchema,
  function: nonEmptyTextSchema,
  variable: nonEmptyTextSchema
}).passthrough();

const editorThemeColorsSchema = z.object({
  workspaceBackground: nonEmptyTextSchema,
  panelBackground: nonEmptyTextSchema,
  editorBackground: nonEmptyTextSchema,
  border: nonEmptyTextSchema,
  foreground: nonEmptyTextSchema,
  muted: nonEmptyTextSchema,
  accent: nonEmptyTextSchema,
  selection: nonEmptyTextSchema,
  diffRemoved: nonEmptyTextSchema,
  diffRemovedStrong: nonEmptyTextSchema,
  diffRemovedWord: nonEmptyTextSchema,
  diffAdded: nonEmptyTextSchema,
  diffAddedStrong: nonEmptyTextSchema,
  diffAddedWord: nonEmptyTextSchema,
  syntax: editorSyntaxColorsSchema
}).passthrough();

const editorSettingsSchema = z.object({
  version: positiveIntegerSchema,
  theme: editorThemeModeSchema,
  defaultEncoding: textEncodingSchema,
  editor: z.object({
    fontFamilies: z.array(nonEmptyTextSchema).min(1),
    fontSize: positiveIntegerSchema,
    lineHeight: positiveIntegerSchema,
    minimap: z.boolean()
  }).passthrough(),
  largeFile: z.object({
    maxBytes: positiveIntegerSchema,
    maxLines: positiveIntegerSchema
  }).passthrough(),
  themes: z.object({
    light: editorThemeColorsSchema,
    dark: editorThemeColorsSchema
  }).passthrough()
}).passthrough();

export const editorSettingsSnapshotSchema: z.ZodType<EditorSettingsSnapshot> = z.object({
  settings: editorSettingsSchema,
  configPath: z.string(),
  warning: z.string().optional()
}).passthrough() as z.ZodType<EditorSettingsSnapshot>;

export const reviewLayoutSizesSchema = z.object({
  left: finiteNumberSchema,
  right: finiteNumberSchema
});
