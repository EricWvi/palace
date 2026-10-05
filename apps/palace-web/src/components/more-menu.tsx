import type { ReactNode } from "react";
import { DropdownMenu } from "radix-ui";

export interface MenuAction {
  label: string;
  onSelect: () => void;
}

// Rare actions wait behind a quiet "···" next to what they act on. The surrounding area reveals
// it on hover (see `.reveals-more` in styles.css); keyboard focus, an open menu and touch screens
// keep it visible. Radix closes the menu on a choice, an outside click or Escape and returns focus.
export function MoreMenu({
  label,
  actions,
  children,
}: {
  label: string;
  actions: MenuAction[];
  children?: ReactNode;
}) {
  return (
    <DropdownMenu.Root modal={false}>
      <DropdownMenu.Trigger className="more" aria-label={label}>
        ···
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          className="menu"
          align="start"
          sideOffset={4}
          // A dialog opened from the menu takes focus itself; returning it to the trigger first
          // would steal it back.
          onCloseAutoFocus={(event) => {
            if (document.querySelector('[role="dialog"]'))
              event.preventDefault();
          }}
        >
          {actions.map((action) => (
            <DropdownMenu.Item key={action.label} onSelect={action.onSelect}>
              {action.label}
            </DropdownMenu.Item>
          ))}
          {children}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
