import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { buildCache, CACHE_FILE, writeCache } from "./notes-cache.ts";

const script = join(dirname(fileURLToPath(import.meta.url)), "notes-cache.ts");
const run = promisify(execFile);

// Writes a folder of notes from relative paths; it is never inside a git repository.
async function folder(files: Record<string, string>) {
  const root = await mkdtemp(join(tmpdir(), "palace-notes-"));
  for (const [file, text] of Object.entries(files)) {
    await mkdir(dirname(join(root, file)), { recursive: true });
    await writeFile(join(root, file), text);
  }
  return root;
}

const note = (title: string, date: string, extra = "") =>
  `---\ntitle: ${title}\ndate: ${date}\n${extra}---\n\n正文\n`;

// specs/test-cases/web/stars/notes.md#cache-script-must-enumerate-the-files-search-sees-and-reject-invalid-frontmatter
test("collects the title and date of exactly the notes ripgrep sees", async () => {
  const root = await folder({
    ".gitignore": "drafts/\n",
    ".ignore": "private.md\n",
    "rust/advisory-lock.md": note(
      "PostgreSQL advisory lock 的用法",
      "2026-10-05",
      "tag: [rust, postgres]\n",
    ),
    "旅行/周末.md": note('"计划: 周末出行"', "2026-09-30T21:15:00+08:00"),
    "drafts/half.md": note("草稿", "2026-10-01"),
    "private.md": note("私人", "2026-10-02"),
    ".hidden/secret.md": note("隐藏", "2026-10-03"),
    "image.png": "not a note",
    "interview/README.md": "# 面试\n",
    "interview/case-template.md": note("<题目>", "YYYY-MM-DD", "draft: true\n"),
    "interview/published.md": note("系统设计", "2026-10-06", "draft: false\n"),
  });
  const { stdout } = await run("node", [script, root]);
  assert.equal(stdout, "已写入 3 篇笔记到 .palace-cache.json，跳过 2 个文件\n");
  assert.deepEqual(JSON.parse(await readFile(join(root, CACHE_FILE), "utf8")), {
    version: 1,
    notes: [
      {
        id: "interview/published",
        title: "系统设计",
        date: "2026-10-06",
      },
      {
        id: "rust/advisory-lock",
        title: "PostgreSQL advisory lock 的用法",
        date: "2026-10-05",
      },
      {
        id: "旅行/周末",
        title: "计划: 周末出行",
        date: "2026-09-30T21:15:00+08:00",
      },
    ],
  });
  // Running again over the folder holding the cache sees the same notes.
  assert.deepEqual(await buildCache(root), {
    cache: JSON.parse(await readFile(join(root, CACHE_FILE), "utf8")),
    skipped: 2,
  });
});

// specs/test-cases/web/stars/notes.md#cache-script-must-enumerate-the-files-search-sees-and-reject-invalid-frontmatter
test("reports every invalid note and keeps the previous cache", async () => {
  const root = await folder({ "good.md": note("好", "2026-10-05") });
  await run("node", [script, root]);
  const previous = await readFile(join(root, CACHE_FILE), "utf8");
  await writeFile(join(root, "untitled.md"), note('""', "2026-10-05"));
  await writeFile(join(root, "bad-date.md"), note("坏日期", "2026-02-30"));
  await writeFile(
    join(root, "local.md"),
    note("无时区", "2026-10-05T10:00:00"),
  );
  await writeFile(join(root, "bare.md"), "# 没有 frontmatter，不是笔记\n");
  await writeFile(join(root, "both.md"), "---\ntag: x\n---\n");
  await writeFile(
    join(root, "maybe-draft.md"),
    note("草稿？", "2026-10-05", "draft: yes\n"),
  );

  const failure = await run("node", [script, root]).then(
    () => assert.fail("the script must fail"),
    (error: { code: number; stderr: string }) => error,
  );
  assert.equal(failure.code, 1);
  assert.deepEqual(failure.stderr.trim().split("\n"), [
    "bad-date.md: date 不是有效的 YYYY-MM-DD 或带时区偏移的 RFC 3339 时间：2026-02-30",
    "both.md: title 必须是非空字符串",
    "both.md: date 必须是 YYYY-MM-DD 或带时区偏移的 RFC 3339 时间",
    "local.md: date 不是有效的 YYYY-MM-DD 或带时区偏移的 RFC 3339 时间：2026-10-05T10:00:00",
    "maybe-draft.md: draft 必须是 true 或 false",
    "untitled.md: title 必须是非空字符串",
    "6 个问题，未写入 .palace-cache.json",
  ]);
  assert.equal(await readFile(join(root, CACHE_FILE), "utf8"), previous);
});

// specs/test-cases/web/stars/notes.md#cache-script-must-enumerate-the-files-search-sees-and-reject-invalid-frontmatter
test("replaces the cache without leaving a temporary file behind", async () => {
  const root = await folder({});
  await writeCache(root, { version: 1, notes: [] });
  await writeCache(root, {
    version: 1,
    notes: [{ id: "a", title: "A", date: "2026-10-05" }],
  });
  assert.deepEqual(await readdir(root), [CACHE_FILE]);
  assert.deepEqual(JSON.parse(await readFile(join(root, CACHE_FILE), "utf8")), {
    version: 1,
    notes: [{ id: "a", title: "A", date: "2026-10-05" }],
  });
});
