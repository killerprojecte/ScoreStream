import type {DocumentMetadata} from '../types';

/** Display only explicit plain metadata. Filenames and product names are never score text. */
export function readMetadata(value: unknown): DocumentMetadata {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  const source = value as Record<string, unknown>;
  const result: DocumentMetadata = {};
  for (const key of ['title', 'description', 'header', 'footer', 'subtitle', 'composer', 'lyricist', 'arranger', 'work_number', 'movement', 'movement_number', 'rights', 'source_description', 'language'] as const) {
    if (typeof source[key] === 'string' && source[key].trim()) result[key] = source[key].trim();
  }
  if (Array.isArray(source.authors)) {
    const authors = source.authors.filter((item): item is string => typeof item === 'string').map(item => item.trim()).filter(Boolean);
    if (authors.length) result.authors = authors;
  }
  return result;
}
