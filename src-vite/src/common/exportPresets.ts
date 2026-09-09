/**
 * Export presets.
 *
 * A preset is the answer to "what shape should these photos come out in" —
 * format, size, quality and whether the camera and location data travel with
 * them. Saving those choices under a name is the point: the same three or four
 * recipes get used over and over, and retyping them every time is where a
 * batch export stops being worth doing.
 *
 * Definitions live in the persisted settings store, so they survive restarts
 * but stay local to this machine rather than travelling with a library backup.
 */
import { config } from '@/common/config';

export type ExportFormat = 'jpeg' | 'png' | 'webp';
export type ConflictPolicy = 'keep_both' | 'replace' | 'skip';

export interface ExportPreset {
  id: string;
  name: string;
  format: ExportFormat;
  /** 1-100. Only JPEG uses it; PNG and WebP are written losslessly. */
  quality: number;
  /** Longest edge in pixels; 0 keeps the decoded size. Never enlarges. */
  maxEdge: number;
  keepMetadata: boolean;
}

export const MAX_EXPORT_PRESETS = 20;
export const PRESET_NAME_MAX = 24;

/** Long-edge sizes offered in the dialog, plus 0 for "original". */
export const EXPORT_EDGE_CHOICES = [0, 1080, 1600, 2048, 3000, 4096] as const;

/** Quality steps, matching the image editor's High / Medium / Low. */
export const EXPORT_QUALITY_CHOICES = [
  { value: 90, key: 'high' },
  { value: 80, key: 'medium' },
  { value: 60, key: 'low' },
] as const;

/**
 * Shipped so the feature is usable before the user has saved anything.
 * `nameKey` is translated at render time; saved presets carry a literal name.
 */
export const BUILT_IN_PRESETS: (Omit<ExportPreset, 'name'> & { nameKey: string })[] = [
  {
    id: 'builtin:share',
    nameKey: 'share',
    format: 'jpeg',
    quality: 80,
    maxEdge: 2048,
    keepMetadata: false,
  },
  {
    id: 'builtin:archive',
    nameKey: 'archive',
    format: 'jpeg',
    quality: 90,
    maxEdge: 0,
    keepMetadata: true,
  },
  {
    id: 'builtin:web',
    nameKey: 'web',
    format: 'webp',
    quality: 80,
    maxEdge: 1600,
    keepMetadata: false,
  },
];

function settings(): any {
  const store = config.settings as any;
  // Persisted settings from an older build have no export block.
  if (!store.exportOptions) {
    store.exportOptions = {
      presets: [],
      lastPresetId: BUILT_IN_PRESETS[0].id,
      lastDestination: '',
      conflict: 'keep_both' as ConflictPolicy,
    };
  }
  return store.exportOptions;
}

export function listPresets(): ExportPreset[] {
  const list = settings().presets;
  return Array.isArray(list) ? list.filter((preset: any) => preset && preset.id) : [];
}

export function findPreset(id: string | null | undefined): ExportPreset | null {
  if (!id) return null;
  return listPresets().find((preset) => preset.id === id) ?? null;
}

export function savePreset(preset: Omit<ExportPreset, 'id'>, id?: string): ExportPreset | null {
  const name = preset.name.trim().slice(0, PRESET_NAME_MAX);
  if (!name) return null;

  const list = listPresets();
  const next: ExportPreset = { ...preset, name, id: id || `user:${Date.now().toString(36)}` };
  const index = list.findIndex((item) => item.id === next.id);
  if (index >= 0) {
    const updated = list.slice();
    updated[index] = next;
    settings().presets = updated;
  } else {
    if (list.length >= MAX_EXPORT_PRESETS) return null;
    settings().presets = [...list, next];
  }
  return next;
}

export function removePreset(id: string): void {
  settings().presets = listPresets().filter((preset) => preset.id !== id);
  if (settings().lastPresetId === id) {
    settings().lastPresetId = BUILT_IN_PRESETS[0].id;
  }
}

export function getLastPresetId(): string {
  return String(settings().lastPresetId || BUILT_IN_PRESETS[0].id);
}

export function setLastPresetId(id: string): void {
  settings().lastPresetId = id;
}

export function getLastDestination(): string {
  return String(settings().lastDestination || '');
}

export function setLastDestination(path: string): void {
  settings().lastDestination = path;
}

export function getConflictPolicy(): ConflictPolicy {
  const value = settings().conflict;
  return value === 'replace' || value === 'skip' ? value : 'keep_both';
}

export function setConflictPolicy(policy: ConflictPolicy): void {
  settings().conflict = policy;
}
