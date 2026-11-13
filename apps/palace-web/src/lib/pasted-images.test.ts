import { expect, it } from "vitest";
import {
  imageMarkdown,
  imageName,
  pastedImages,
  placeholder,
  shiftCaret,
} from "./pasted-images";

// specs/test-cases/web/echo/echo-page.md#a-pasted-picture-must-become-palace-media-in-place
it("takes only the image files off a paste and names them", () => {
  const shot = new File(["x"], "image.png", { type: "image/png" });
  const photo = new File(["x"], "猫 [1].jpg", { type: "image/jpeg" });
  const unnamed = new File(["x"], "", { type: "image/webp" });
  const text = new File(["x"], "notes.txt", { type: "text/plain" });
  const data = { files: [shot, text, photo] } as unknown as DataTransfer;
  expect(pastedImages(data)).toEqual([shot, photo]);
  expect([shot, photo, unnamed].map(imageName)).toEqual([
    "image.png",
    "猫 [1].jpg",
    "image.webp",
  ]);
  // Brackets in a name must not end the alt text.
  expect(imageMarkdown("猫 [1].jpg", "0192")).toBe(
    "![猫 \\[1\\].jpg](/api/media/0192)",
  );
  expect(placeholder("image.png", 3)).toBe("![上传中 image.png…](uploading-3)");
});

it("keeps a caret on the same words when a span before it changes length", () => {
  // `from` 5 characters at 10 become 8.
  expect([4, 10, 12, 15, 20].map((at) => shiftCaret(at, 10, 5, 8))).toEqual([
    4, 10, 18, 18, 23,
  ]);
});
