import assert from "node:assert/strict";
import { responseValidator } from "./api-responses.mjs";

let input = "";
for await (const chunk of process.stdin) input += chunk;
const { document, samples, requireAllOperations } = JSON.parse(input);
assert.ok(samples.length, "HTTP capture must not be empty");
const validate = responseValidator(document);
for (const sample of samples) validate(sample);
const seen = new Set(
  samples.map(({ method, path }) => `${method.toLowerCase()} ${path}`),
);
if (requireAllOperations) {
  for (const [path, item] of Object.entries(document.paths)) {
    for (const method of Object.keys(item)) {
      if (
        [
          "get",
          "post",
          "put",
          "delete",
          "patch",
          "head",
          "options",
          "trace",
        ].includes(method)
      )
        assert.ok(
          seen.has(`${method} ${path}`),
          `No real HTTP sample for ${method} ${path}`,
        );
    }
  }
}
console.log(
  `Validated ${samples.length} real HTTP responses across ${seen.size} operations.`,
);
