import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import openapiTS, { astToString, COMMENT_HEADER } from "openapi-typescript";
import { format } from "prettier";
import ts from "typescript";

const root = fileURLToPath(new URL("../", import.meta.url));
const document = JSON.parse(
  await readFile(path.join(root, "contracts", "openapi.json"), "utf8"),
);
// Origin is mandatory on the wire but controlled by the browser, not by application code.
for (const item of Object.values(document.paths)) {
  for (const operation of Object.values(item)) {
    if (
      operation &&
      typeof operation === "object" &&
      Array.isArray(operation.parameters)
    ) {
      operation.parameters = operation.parameters.filter(
        (parameter) =>
          !(
            parameter.in === "header" &&
            parameter.name.toLowerCase() === "origin"
          ),
      );
    }
  }
}
const nodes = await openapiTS(document, {
  transform(schema) {
    if (schema.format === "binary")
      return ts.factory.createTypeReferenceNode("Blob");
  },
});
const output = path.join(
  root,
  "apps",
  "palace-web",
  "src",
  "lib",
  "generated",
  "api.ts",
);
await mkdir(path.dirname(output), { recursive: true });
await writeFile(
  output,
  await format(
    COMMENT_HEADER +
      "// Regenerate with task api:generate. Browser supplies Origin; binary parts accept Blob/File.\n" +
      astToString(nodes),
    { parser: "typescript", printWidth: 80 },
  ),
);
console.log("Generated apps/palace-web/src/lib/generated/api.ts");
