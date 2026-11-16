import {t} from './i18n';
/** Native diagnostics are data, not HTML. Keep technical detail without dumping JSON into the UI. */
export function describeError(error:unknown):string {
 const text=error instanceof Error?error.message:String(error);
 try {const parsed=JSON.parse(text);const diagnostics:unknown=Array.isArray(parsed)?parsed:parsed?.diagnostics;if(Array.isArray(diagnostics)&&diagnostics.length&&diagnostics.every(d=>d&&typeof d.message==='string'))return diagnostics.slice(0,3).map(d=>Number.isInteger(d.line)?t('Line {line}: {message}',{line:d.line,message:d.message}):d.path?`${d.feature??'Import'} · ${d.path}${d.source?` (${d.source.line}:${d.source.column})`:''}: ${d.message}`:d.message).join(' · ');}catch{}
 return t(text);
}
