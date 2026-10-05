/*
 * Shared markup for the prototypes in design.
 *
 * Plain script, not an ES module: the pages are opened straight from disk,
 * and browsers block module scripts and fetch() on file:// URLs. Custom
 * elements work there and keep each page's markup to a single tag.
 */

const NAV_ITEMS = ["时刻", "行事", "旅途", "摘星", "回响"];

// Inlined rather than an <img> so the ornament can take its color from CSS.
const ORNAMENT = `
<svg class="ornament" viewBox="${globalThis.palaceOrnament.viewBox}" fill="currentColor" aria-hidden="true">
  <path fill-rule="evenodd"
    d="${globalThis.palaceOrnament.path}" />
</svg>`;

/*
 * Top bar: <palace-header active="时刻"></palace-header>.
 * `active` names the current nav item; it gets aria-current="page".
 */
class PalaceHeader extends HTMLElement {
  connectedCallback() {
    const active = this.getAttribute("active");
    const links = NAV_ITEMS.map((label) => {
      const current = label === active ? ' aria-current="page"' : "";
      return `<a href="#"${current}>${label}</a>`;
    }).join("");
    this.innerHTML = `<header class="top">${ORNAMENT}<nav>${links}</nav></header>`;
  }
}

customElements.define("palace-header", PalaceHeader);
