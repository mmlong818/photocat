/**
 * User-defined subjects.
 *
 * The built-in subjects in `smartTags.ts` are eight fixed CLIP prompts. They
 * cover the common cases and nothing else, so this adds a user-owned list
 * alongside them: a name to show in the sidebar and a sentence describing what
 * the photo looks like, which is encoded by the same CLIP text model that
 * powers the search box. No indexing is involved — a subject is just a saved
 * query, so it starts working the moment it is saved and stays correct as the
 * library grows.
 *
 * Definitions live in the persisted settings store, so they survive restarts
 * but are local to this machine rather than part of a library backup.
 */
import { config } from '@/common/config';
import { getSmartTagById, type SmartTagDef } from '@/common/smartTags';

/** Custom subject ids carry this prefix so they can never collide with a built-in. */
export const CUSTOM_SUBJECT_PREFIX = 'custom:';

/** Keep the sidebar list manageable and the settings blob small. */
export const MAX_CUSTOM_SUBJECTS = 40;

export const SUBJECT_NAME_MAX = 24;
export const SUBJECT_PROMPT_MAX = 240;

/**
 * How closely a photo must match before it is counted. Semantic similarity
 * scores sit in a narrow band, so these are deliberately close together;
 * a tenth of a point is the difference between "a few photos" and "most of
 * the library".
 */
export const SUBJECT_PRECISION = {
  strict: 0.28,
  standard: 0.25,
  loose: 0.22,
} as const;

export type SubjectPrecision = keyof typeof SUBJECT_PRECISION;

export interface CustomSubject {
  id: string;
  name: string;
  prompt: string;
  precision: SubjectPrecision;
}

function store(): CustomSubject[] {
  const list = (config.settings as any).customSubjects;
  return Array.isArray(list) ? list : [];
}

export function listCustomSubjects(): CustomSubject[] {
  return store().filter(
    (subject) => subject && typeof subject.id === 'string' && typeof subject.name === 'string',
  );
}

export function isCustomSubjectId(id: string | null | undefined): boolean {
  return typeof id === 'string' && id.startsWith(CUSTOM_SUBJECT_PREFIX);
}

export function findCustomSubject(id: string | null | undefined): CustomSubject | null {
  if (!id) return null;
  return listCustomSubjects().find((subject) => subject.id === id) ?? null;
}

/**
 * Resolve a sidebar subject id to something the search can run, whether it is
 * one of the built-ins or one the user wrote. Replaces a direct call to
 * `getSmartTagById`.
 */
export function resolveSubject(id: string | null | undefined): SmartTagDef | null {
  const custom = findCustomSubject(id);
  if (custom) {
    return {
      id: custom.id,
      prompt: custom.prompt,
      threshold: SUBJECT_PRECISION[custom.precision] ?? SUBJECT_PRECISION.standard,
    };
  }
  return getSmartTagById(id);
}

function newId(): string {
  const random = Math.random().toString(36).slice(2, 10);
  return `${CUSTOM_SUBJECT_PREFIX}${Date.now().toString(36)}${random}`;
}

export function isValidSubject(name: string, prompt: string): boolean {
  return name.trim().length > 0 && prompt.trim().length > 0;
}

/** Adds a subject and returns it, or null when the list is full or invalid. */
export function addCustomSubject(
  name: string,
  prompt: string,
  precision: SubjectPrecision = 'standard',
): CustomSubject | null {
  const list = listCustomSubjects();
  if (list.length >= MAX_CUSTOM_SUBJECTS || !isValidSubject(name, prompt)) return null;

  const subject: CustomSubject = {
    id: newId(),
    name: name.trim().slice(0, SUBJECT_NAME_MAX),
    prompt: prompt.trim().slice(0, SUBJECT_PROMPT_MAX),
    precision,
  };
  (config.settings as any).customSubjects = [...list, subject];
  return subject;
}

export function updateCustomSubject(
  id: string,
  name: string,
  prompt: string,
  precision: SubjectPrecision,
): boolean {
  if (!isValidSubject(name, prompt)) return false;
  const list = listCustomSubjects();
  const index = list.findIndex((subject) => subject.id === id);
  if (index < 0) return false;

  const next = list.slice();
  next[index] = {
    id,
    name: name.trim().slice(0, SUBJECT_NAME_MAX),
    prompt: prompt.trim().slice(0, SUBJECT_PROMPT_MAX),
    precision,
  };
  (config.settings as any).customSubjects = next;
  return true;
}

export function removeCustomSubject(id: string): void {
  (config.settings as any).customSubjects = listCustomSubjects().filter(
    (subject) => subject.id !== id,
  );
}
