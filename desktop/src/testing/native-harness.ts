/** Helpers for the real Tauri/WebKit smoke run. No jsdom geometry or mocked host. */
export type Assert = (condition: unknown, name: string) => void;
export const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
export async function until(predicate: () => boolean, description = 'native UI state', timeout = 30000) {
  const start = Date.now();
  while (!predicate()) {
    if (Date.now() - start > timeout) throw Error(`Timed out waiting for ${description}`);
    await delay(30);
  }
}
export async function paint() {
  await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
}
export function get<T extends HTMLElement = HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) throw Error(`Native smoke target is not mounted: #${id}`);
  return node as T;
}
export function visible(id: string) {
  const node = document.getElementById(id);
  return !!node && !node.hidden && node.getClientRects().length > 0 && getComputedStyle(node).visibility !== 'hidden';
}
export async function click(id: string) { get(id).focus(); get(id).click(); await paint(); }
export async function key(node: Element, key: string) {
  node.dispatchEvent(new KeyboardEvent('keydown', {key, bubbles: true, cancelable: true}));
  await paint();
}
export async function input(node: HTMLInputElement, value: string) {
  // Bypass React's value tracker exactly as a native keystroke does; writing
  // node.value alone does not notify a controlled React field.
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(node, value);
  node.dispatchEvent(new Event('input', {bubbles: true}));
  await paint();
}
