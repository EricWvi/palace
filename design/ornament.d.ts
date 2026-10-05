export {};

declare global {
  /** Shared SVG geometry initialized by importing ornament.js. */
  var palaceOrnament: Readonly<{
    viewBox: string;
    path: string;
  }>;
}
