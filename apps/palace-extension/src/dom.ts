// Builds an element with its properties and children: the extension's pages are a few lines of
// text and buttons, not worth a framework.
export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  properties: Record<string, unknown>,
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const made = document.createElement(tag);
  for (const [name, value] of Object.entries(properties))
    (made as unknown as Record<string, unknown>)[name] = value;
  made.append(...children);
  return made;
}
