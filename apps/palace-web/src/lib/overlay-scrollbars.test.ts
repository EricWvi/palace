import { expect, it } from "vitest";
import { installOverlayScrollbars, thumbGeometry } from "./overlay-scrollbars";

it("places the thumb in proportion to the scroll and keeps a grabbable size", () => {
  expect(thumbGeometry(800, 3200, 1200)).toEqual({
    size: 200,
    offset: 300,
    ratio: 4,
  });
  expect(thumbGeometry(800, 80000, 0)).toEqual({
    size: 32,
    offset: 0,
    ratio: 79200 / 768,
  });
  // Overscroll past either end pins the thumb instead of running off the track.
  expect(thumbGeometry(800, 3200, 5000)?.offset).toBe(600);
  expect(thumbGeometry(800, 3200, -40)?.offset).toBe(0);
});

it("draws nothing when there is nothing to scroll", () => {
  expect(thumbGeometry(800, 800, 0)).toBeNull();
});

it("leaves overlay scrollbars to the platform", () => {
  const uninstall = installOverlayScrollbars();
  expect(document.documentElement.dataset.scrollbar).toBeUndefined();
  uninstall();
});
