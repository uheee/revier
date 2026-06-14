import type { RelatedCommit, TouchedRange } from '../../shared/reviewTypes';
import type { BlameLine } from '../git/analysisGitClient';
export interface AnalysisCache { commits: Map<string, RelatedCommit>; parents: Map<string, string[]>; blameRanges: Map<string, BlameLine[]>; patches: Map<string, string>; touchedRanges: Map<string, TouchedRange[]>; }
export function createAnalysisCache(): AnalysisCache { return { commits: new Map(), parents: new Map(), blameRanges: new Map(), patches: new Map(), touchedRanges: new Map() }; }
export function cacheKey(parts: Array<string | number | undefined>): string { return parts.map((part) => String(part ?? '')).join('\u0000'); }
