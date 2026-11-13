// Grows a textarea to its text. Measuring needs the field collapsed to `auto` first, and that
// forced layout shrinks every scrolling ancestor's content: a card capped by `max-height` (or the
// page itself, `<html>`) clamps its scroll to the top, and once the height is back the browser
// only scrolls the caret just into view, leaving it on the bottom line after every keystroke.
// The ancestors' offsets are taken before collapsing and given back after, so the view stays.
export function fitTextarea(element: HTMLTextAreaElement) {
  const scrolled: [Element, number][] = [];
  for (let node = element.parentElement; node; node = node.parentElement) {
    if (node.scrollTop > 0) scrolled.push([node, node.scrollTop]);
  }
  element.style.height = "auto";
  element.style.height = `${element.scrollHeight}px`;
  for (const [node, top] of scrolled) node.scrollTop = top;
}
