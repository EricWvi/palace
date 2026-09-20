import { execFileSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
// Finish compilation and serialization before touching the committed contract.
const output = execFileSync(
  "cargo",
  [
    "run",
    "--locked",
    "--quiet",
    "-p",
    "palace-backend",
    "--example",
    "export_openapi",
  ],
  { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
);
JSON.parse(output);
const directory = path.join(root, "contracts");
await mkdir(directory, { recursive: true });
await writeFile(path.join(directory, "openapi.json"), output);
console.log("Exported contracts/openapi.json");
