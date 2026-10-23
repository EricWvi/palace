import { useRef, type ReactNode } from "react";
import { ContextMenu, DropdownMenu } from "radix-ui";

// One line of a task's or a list's menu. The same entries feed the right-click menu and the
// `···` menu, so the two never drift apart.
export type MenuEntry =
  | {
      kind: "item";
      label: string;
      onSelect: () => void;
      danger?: boolean;
      current?: boolean;
    }
  | { kind: "sub"; label: string; entries: MenuEntry[] }
  | { kind: "separator" };

// Both Radix menus share one shape of parts; this names the ones the entries need.
type Parts = typeof DropdownMenu | typeof ContextMenu;

function Entries({
  parts: M,
  entries,
}: {
  parts: Parts;
  entries: MenuEntry[];
}) {
  return entries.map((entry, i) => {
    if (entry.kind === "separator")
      return <M.Separator key={i} className="menu-separator" />;
    if (entry.kind === "sub")
      return (
        <M.Sub key={entry.label}>
          <M.SubTrigger className="menu-sub">{entry.label}</M.SubTrigger>
          <M.Portal>
            <M.SubContent className="menu" sideOffset={4}>
              <Entries parts={M} entries={entry.entries} />
            </M.SubContent>
          </M.Portal>
        </M.Sub>
      );
    return (
      <M.Item
        key={entry.label}
        className={entry.danger ? "danger" : undefined}
        aria-current={entry.current || undefined}
        onSelect={entry.onSelect}
      >
        {entry.label}
      </M.Item>
    );
  });
}

// Right-clicking anywhere on `children` opens the entries at the pointer.
export function RightClickMenu({
  entries,
  children,
}: {
  entries: MenuEntry[];
  children: ReactNode;
}) {
  return (
    <ContextMenu.Root modal={false}>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="menu">
          <Entries parts={ContextMenu} entries={entries} />
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

// The quiet `···` at the end of a row, revealed on hover, opening the same entries.
export function MoreEntries({
  label,
  entries,
}: {
  label: string;
  entries: MenuEntry[];
}) {
  // A pointer-driven close parks focus beside the trigger, as MoreMenu does, so the dots are
  // not left ringed after a click.
  const input = useRef<"pointer" | "keyboard">("keyboard");
  const rest = useRef<HTMLSpanElement>(null);
  return (
    <DropdownMenu.Root modal={false}>
      <DropdownMenu.Trigger
        className="more"
        aria-label={label}
        onPointerDown={(event) => {
          input.current = "pointer";
          // A press on the dots must not start dragging the row they sit in.
          event.stopPropagation();
        }}
        onKeyDown={(event) => {
          input.current = "keyboard";
          event.stopPropagation();
        }}
      >
        ···
      </DropdownMenu.Trigger>
      <span ref={rest} className="focus-rest" tabIndex={-1} />
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          className="menu"
          align="end"
          sideOffset={4}
          onCloseAutoFocus={(event) => {
            if (
              document.querySelector('[role="dialog"]') ||
              input.current === "pointer"
            ) {
              event.preventDefault();
              if (input.current === "pointer") rest.current?.focus();
            }
          }}
        >
          <Entries parts={DropdownMenu} entries={entries} />
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
