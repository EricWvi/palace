import { expect, it } from "vitest";
import { validateFile } from "./import-file";
it.each([
  ["[]", "empty.json", "非空"],
  ["{}", "object.json", "非空"],
  ['[{"role":"tool","content":"x"}]', "role.json", "第 1 条"],
  ['[{"role":"user","content":1}]', "content.json", "第 1 条"],
  ["not json", "text.txt", ".json"],
])("rejects unsupported input %s", async (content, name, message) => {
  await expect(validateFile(new File([content], name))).rejects.toThrow(
    message,
  );
});
it("rejects oversized files before reading and preserves empty content and exporter metadata", async () => {
  const oversized = new File(["x"], "large.json");
  Object.defineProperty(oversized, "size", { value: 8 * 1024 * 1024 + 1 });
  await expect(validateFile(oversized)).rejects.toThrow("8 MiB");
  await expect(
    validateFile(
      new File(['[{"role":"user","content":"","extra":42}]'], "valid.json"),
    ),
  ).resolves.toBeUndefined();
});
