/*
 * PROTOTYPE ONLY: a floating bar that flips a page between design variants.
 *
 *   <variant-switcher variants="A:书目,B:目录,C:寻找"></variant-switcher>
 *
 * The current key lives in ?variant= so a variant survives reloads and can be
 * shared, and is mirrored onto <html data-variant>. Pages re-render on the
 * bubbling `variantchange` event. ← and → cycle unless the reader is typing.
 * Not part of the design being judged, so it is drawn inverted to stand apart.
 */
class VariantSwitcher extends HTMLElement {
  connectedCallback() {
    const variants = this.getAttribute("variants").split(",").map((entry) => {
      const [key, name] = entry.split(":");
      return { key, name };
    });
    const params = new URLSearchParams(location.search);
    let index = Math.max(0, variants.findIndex((v) => v.key === params.get("variant")));

    this.innerHTML = `
      <style>
        variant-switcher {
          position: fixed;
          left: 50%;
          bottom: 20px;
          z-index: 10;
          display: flex;
          align-items: center;
          gap: 4px;
          padding: 4px;
          transform: translateX(-50%);
          background: var(--text);
          color: var(--bg);
          border-radius: var(--radius-md);
          box-shadow: 0 6px 20px rgb(0 0 0 / .2);
          font-family: var(--font-ui);
          font-size: var(--font-size-sm);
        }
        variant-switcher button {
          padding: 4px 10px;
          border-radius: var(--radius-sm);
        }
        variant-switcher button:hover {
          background: color-mix(in srgb, var(--bg) 15%, transparent);
        }
        variant-switcher span {
          min-width: 120px;
          text-align: center;
        }
      </style>
      <button data-step="-1" aria-label="上一个变体">←</button>
      <span></span>
      <button data-step="1" aria-label="下一个变体">→</button>`;
    const label = this.querySelector("span");

    const apply = () => {
      const { key, name } = variants[index];
      label.textContent = `${key} · ${name}`;
      document.documentElement.dataset.variant = key;
    };
    const go = (step) => {
      index = (index + step + variants.length) % variants.length;
      params.set("variant", variants[index].key);
      history.replaceState(null, "", `?${params}`);
      apply();
      this.dispatchEvent(new CustomEvent("variantchange", { bubbles: true }));
    };

    this.addEventListener("click", (e) => {
      const button = e.target.closest("[data-step]");
      if (button) go(Number(button.dataset.step));
    });
    document.addEventListener("keydown", (e) => {
      const typing = e.target.closest("input, textarea, [contenteditable]");
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "ArrowLeft") go(-1);
      if (e.key === "ArrowRight") go(1);
    });
    apply();
  }
}

customElements.define("variant-switcher", VariantSwitcher);
