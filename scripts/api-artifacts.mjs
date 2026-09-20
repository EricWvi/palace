import { readFile } from "node:fs/promises";
import path from "node:path";

// Reading and comparing is deliberately separate from generation: a failed check cannot repair drift.
export async function staleArtifacts(root, artifacts) {
  const stale = [];
  for (const [file, expected] of artifacts) {
    let actual;
    try {
      actual = await readFile(path.join(root, file), "utf8");
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    if (actual !== expected) stale.push(file);
  }
  return stale;
}
