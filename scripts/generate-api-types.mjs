import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { browserTypes } from "./api-types.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const document = JSON.parse(
  await readFile(path.join(root, "contracts", "openapi.json"), "utf8"),
);
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
await writeFile(output, await browserTypes(document));
console.log("Generated apps/palace-web/src/lib/generated/api.ts");
