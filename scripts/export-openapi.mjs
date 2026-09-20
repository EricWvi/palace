import { exportContract } from "./api-export.mjs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = exportContract(root);
const directory = path.join(root, "contracts");
await mkdir(directory, { recursive: true });
await writeFile(path.join(directory, "openapi.json"), output);
console.log("Exported contracts/openapi.json");
