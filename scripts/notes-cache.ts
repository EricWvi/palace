// Builds the cache Palace reads its 摘星 notes from: the `title` and `date` of every Markdown
// note in a folder, written atomically to `<folder>/.palace-cache.json`.
//
//   node scripts/notes-cache.ts <notes-folder>
//
// Palace trusts the cache and never parses a note's frontmatter itself, so every rule a note
// must meet is checked here; one bad note fails the whole run and leaves the old cache alone.
// See specs/decisions/web/stars/20261014-notes-from-mounted-folder.md.
import { execFile } from "node:child_process";
import { readFile, rename, rm, writeFile } from "node:fs/promises";
import { join, sep } from "node:path";
import { promisify } from "node:util";
import { parse } from "yaml";

export const CACHE_FILE = ".palace-cache.json";

export type NoteEntry = { id: string; title: string; date: string };
export type Cache = { version: 1; notes: NoteEntry[] };
export type Problem = { file: string; message: string };

const run = promisify(execFile);

// Lists notes with the same ripgrep rules Palace searches with, so the list and the search see
// one set of files: `.gitignore` applies even outside a git repository, hidden files never do.
// A file type selects `*.md` rather than `--glob`, which would override the ignore files, and a
// personal ripgrep configuration must not change which files count.
export async function listNoteFiles(folder: string): Promise<string[]> {
  try {
    const { stdout } = await run(
      "rg",
      [
        "--no-config",
        "--files",
        "--no-require-git",
        "--null",
        "--type-add",
        "note:*.md",
        "--type",
        "note",
        ".",
      ],
      { cwd: folder, maxBuffer: 64 * 1024 * 1024 },
    );
    return stdout
      .split("\0")
      .filter(Boolean)
      .map((file) => file.replace(/^\.\//, "").split(sep).join("/"))
      .sort();
  } catch (error) {
    // ripgrep exits with 1 when the folder holds no matching file at all.
    if ((error as { code?: unknown }).code === 1) return [];
    throw error;
  }
}

const day = /^(\d{4})-(\d{2})-(\d{2})$/;
// RFC 3339 with seconds and an explicit offset: what Palace parses without guessing a zone.
const instant =
  /^(\d{4})-(\d{2})-(\d{2})T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

// A `YYYY-MM-DD` that names a real calendar day, so Palace can read it in its own time zone.
function isCalendarDay(year: string, month: string, date: string) {
  const at = new Date(Date.UTC(+year, +month - 1, +date));
  return (
    at.getUTCFullYear() === +year &&
    at.getUTCMonth() === +month - 1 &&
    at.getUTCDate() === +date
  );
}

// Returns why a frontmatter `date` is unusable, or null when Palace can read it.
function dateProblem(value: unknown): string | null {
  if (typeof value !== "string")
    return "date 必须是 YYYY-MM-DD 或带时区偏移的 RFC 3339 时间";
  const parts = day.exec(value) ?? instant.exec(value);
  if (
    !parts ||
    !isCalendarDay(parts[1]!, parts[2]!, parts[3]!) ||
    Number.isNaN(Date.parse(value))
  )
    return `date 不是有效的 YYYY-MM-DD 或带时区偏移的 RFC 3339 时间：${value}`;
  return null;
}

// Reads the note's leading `---` block. The YAML core schema keeps `2026-10-05` a string
// instead of turning it into a UTC midnight, which would move the day across time zones.
export function readFrontmatter(
  text: string,
): Record<string, unknown> | string {
  const lines = text.replace(/^﻿/, "").split(/\r?\n/);
  if (lines[0] !== "---") return "缺少 frontmatter";
  const end = lines.findIndex(
    (line, index) => index > 0 && (line === "---" || line === "..."),
  );
  if (end < 0) return "frontmatter 没有结束行";
  try {
    const value: unknown = parse(lines.slice(1, end).join("\n"), {
      schema: "core",
    });
    if (typeof value !== "object" || value === null || Array.isArray(value))
      return "frontmatter 必须是键值对";
    return value as Record<string, unknown>;
  } catch (error) {
    return `frontmatter 不是有效的 YAML：${(error as Error).message}`;
  }
}

// Checks one note and returns its cache entry, or every problem it has.
export function noteEntry(file: string, text: string): NoteEntry | Problem[] {
  const frontmatter = readFrontmatter(text);
  if (typeof frontmatter === "string") return [{ file, message: frontmatter }];
  const problems: Problem[] = [];
  const { title, date } = frontmatter;
  if (typeof title !== "string" || title.trim() === "")
    problems.push({ file, message: "title 必须是非空字符串" });
  const invalidDate = dateProblem(date);
  if (invalidDate) problems.push({ file, message: invalidDate });
  if (problems.length > 0) return problems;
  return {
    id: file.replace(/\.md$/, ""),
    title: (title as string).trim(),
    date: date as string,
  };
}

// Collects every note of the folder, or all problems found so they can be fixed in one pass.
export async function buildCache(
  folder: string,
): Promise<{ cache: Cache } | { problems: Problem[] }> {
  const notes: NoteEntry[] = [];
  const problems: Problem[] = [];
  for (const file of await listNoteFiles(folder)) {
    const entry = noteEntry(file, await readFile(join(folder, file), "utf8"));
    if (Array.isArray(entry)) problems.push(...entry);
    else notes.push(entry);
  }
  return problems.length > 0 ? { problems } : { cache: { version: 1, notes } };
}

// Replaces the cache by rename, so Palace never reads a half-written file.
export async function writeCache(folder: string, cache: Cache) {
  const target = join(folder, CACHE_FILE);
  const temporary = `${target}.${process.pid}.tmp`;
  try {
    await writeFile(temporary, `${JSON.stringify(cache, null, 2)}\n`);
    await rename(temporary, target);
  } finally {
    await rm(temporary, { force: true });
  }
}

async function main(argv: string[]) {
  const [folder] = argv;
  if (!folder || argv.length !== 1) {
    console.error("用法：node scripts/notes-cache.ts <笔记文件夹>");
    return 2;
  }
  const result = await buildCache(folder);
  if ("problems" in result) {
    for (const { file, message } of result.problems)
      console.error(`${file}: ${message}`);
    console.error(`${result.problems.length} 个问题，未写入 ${CACHE_FILE}`);
    return 1;
  }
  await writeCache(folder, result.cache);
  console.log(`已写入 ${result.cache.notes.length} 篇笔记到 ${CACHE_FILE}`);
  return 0;
}

if (import.meta.main) process.exitCode = await main(process.argv.slice(2));
