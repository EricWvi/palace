import { staleArtifacts } from "./api-artifacts.mjs";
import { fileURLToPath } from "node:url";
import { exportContract } from "./api-export.mjs";
import { browserTypes } from "./api-types.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const contract = exportContract(root);
const artifacts = new Map([
  ["contracts/openapi.json", contract],
  [
    "apps/palace-web/src/lib/generated/api.ts",
    await browserTypes(JSON.parse(contract)),
  ],
]);
const stale = await staleArtifacts(root, artifacts);
for (const file of stale) {
  console.error(
    `Missing or stale generated artifact: ${file}; run task api:generate and commit both artifacts.`,
  );
}
if (stale.length) process.exitCode = 1;
else
  console.log(
    "OpenAPI and browser types match their sources; no files were written.",
  );
