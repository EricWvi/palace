export async function validateFile(file: File): Promise<void> {
  if (!file.name.toLowerCase().endsWith(".json"))
    throw new Error("请选择 .json 文件。");
  if (file.size > 8 * 1024 * 1024) throw new Error("JSON 文件不能超过 8 MiB。");
  let data: unknown;
  try {
    data = JSON.parse(await file.text());
  } catch {
    throw new Error("文件不是有效的 JSON。");
  }
  if (!Array.isArray(data) || data.length === 0)
    throw new Error("JSON 必须是非空的消息数组。");
  if (data.length > 10_000) throw new Error("消息数量不能超过 10,000 条。");
  for (const [index, item] of data.entries()) {
    if (
      !item ||
      !["user", "assistant"].includes(item.role) ||
      typeof item.content !== "string"
    )
      throw new Error(
        `第 ${index + 1} 条消息需要 role（user / assistant）和字符串 content。`,
      );
    if (new TextEncoder().encode(item.content).length > 1024 * 1024)
      throw new Error(`第 ${index + 1} 条消息超过 1 MiB。`);
  }
}
