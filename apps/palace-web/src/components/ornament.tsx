import "../../../../design/ornament.js";

// Keep the SVG inline so its color follows the shared ornament token.
export function Ornament() {
  return (
    <svg
      className="ornament"
      viewBox={globalThis.palaceOrnament.viewBox}
      fill="currentColor"
      aria-hidden="true"
    >
      <path fillRule="evenodd" d={globalThis.palaceOrnament.path} />
    </svg>
  );
}
