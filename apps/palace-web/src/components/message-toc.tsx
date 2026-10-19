import { useMemo } from "react";
import type { Message } from "@/lib/api";
import { ReadingToc, type TocItem } from "./reading-toc";

// The element id a message of the reading page carries, so the table of contents can jump to it.
export function messageAnchor(message: Message) {
  return `message-${message.id}`;
}

// Message order and role come from the current path; navigation behavior is shared with notes.
export function MessageToc({ messages }: { messages: Message[] }) {
  const items = useMemo<TocItem[]>(
    () =>
      messages.map((message) => ({
        id: messageAnchor(message),
        label: message.toc_line || "（空消息）",
        kind: "message",
        role: message.role,
      })),
    [messages],
  );
  return <ReadingToc items={items} label="消息目录" />;
}
