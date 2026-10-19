import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { generateManifest } from "./pal-release-manifest.mjs";

// Core test: specs/test-cases/cli/update.md#updates-must-verify-the-release-before-replacing-the-executable
test("manifest records the actual archives with version-pinned URLs and SHA-256", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "pal-release-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const archives = [
    ["aarch64-apple-darwin", "tar.gz", Buffer.from("macOS archive")],
    ["x86_64-pc-windows-msvc", "zip", Buffer.from("Windows archive")],
  ];
  for (const [target, extension, bytes] of archives) {
    await writeFile(
      join(directory, `pal-v1.2.3-${target}.${extension}`),
      bytes,
    );
  }
  await writeFile(join(directory, "SHA256SUMS"), "ignored");
  await writeFile(
    join(directory, "pal-v0.1.0-other.tar.gz"),
    "ignored old release",
  );
  const expected = {
    version: "v1.2.3",
    assets: archives.map(([target, extension, bytes]) => {
      const name = `pal-v1.2.3-${target}.${extension}`;
      return {
        target,
        name,
        url: `https://github.com/EricWvi/palace/releases/download/v1.2.3/${name}`,
        sha256: createHash("sha256").update(bytes).digest("hex"),
      };
    }),
  };
  assert.deepEqual(
    await generateManifest(directory, "EricWvi/palace", "v1.2.3"),
    expected,
  );
  assert.deepEqual(
    JSON.parse(await readFile(join(directory, "latest.json"), "utf8")),
    expected,
  );
});

test("missing archives and duplicate platforms fail the release manifest step", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "pal-release-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  await assert.rejects(
    generateManifest(directory, "EricWvi/palace", "v1.2.3"),
    /No pal release archives/,
  );
  for (const extension of ["tar.gz", "zip"]) {
    await writeFile(
      join(directory, `pal-v1.2.3-same-target.${extension}`),
      "archive",
    );
  }
  await assert.rejects(
    generateManifest(directory, "EricWvi/palace", "v1.2.3"),
    /duplicate release target/,
  );
  await assert.rejects(
    generateManifest(directory, "EricWvi/palace", "v../other"),
    /Invalid release/,
  );
});
