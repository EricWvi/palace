import { describe, expect, it } from "vitest";
import { fitTextarea } from "./fit-textarea";

// jsdom does no layout, so the card stands in for a browser's: reading the field's scrollHeight
// forces layout, and while the field is collapsed to `auto` that layout clamps the card's scroll
// to the top, as a `max-height` card does in a real page.
function cardWithField(scrollTop: number) {
  const card = document.createElement("div");
  const field = document.createElement("textarea");
  card.append(field);
  let top = scrollTop;
  Object.defineProperty(card, "scrollTop", {
    get: () => top,
    set: (value: number) => {
      top = value;
    },
  });
  Object.defineProperty(field, "scrollHeight", {
    get: () => {
      if (field.style.height === "auto") top = 0;
      return 900;
    },
  });
  return { card, field };
}

describe("fitTextarea", () => {
  it("grows the field to its text and keeps a scrolled card where it was", () => {
    const { card, field } = cardWithField(420);
    fitTextarea(field);
    expect({ height: field.style.height, top: card.scrollTop }).toEqual({
      height: "900px",
      top: 420,
    });
  });

  it("leaves a card at its top untouched", () => {
    const { card, field } = cardWithField(0);
    fitTextarea(field);
    expect({ height: field.style.height, top: card.scrollTop }).toEqual({
      height: "900px",
      top: 0,
    });
  });
});
