import { useRef, type ReactNode } from "react";
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
  onApproach,
  children,
}: {
  label: string;
  actions: MenuAction[];
  // Runs when a pointer or focus reaches the trigger, before the menu opens, so an action can
  // start fetching what it needs while the reader is still choosing.
  onApproach?: () => void;
  children?: ReactNode;
}) {
  // Whichever input last drove the menu decides where focus goes when it closes.
  const input = useRef<"pointer" | "keyboard">("keyboard");
  const outside = useRef(false);
  const rest = useRef<HTMLSpanElement>(null);
  return (
    <DropdownMenu.Root modal={false}>
      <DropdownMenu.Trigger
        className="more"
        aria-label={label}
        onPointerEnter={onApproach}
        onFocus={onApproach}
        onPointerDown={() => (input.current = "pointer")}
        onKeyDown={() => (input.current = "keyboard")}
      >
        ···
      </DropdownMenu.Trigger>
      {/*
       * Radix closes the menu when the window loses focus, e.g. on switching browser tabs, and
       * hands focus back to the trigger while the tab is hidden. Chrome then paints that focus
       * as :focus-visible when the tab comes back, ringing a "···" the reader only clicked. A
       * pointer-driven close parks focus here instead: right after the trigger, so Tab still
       * continues from the same place, but with nothing to draw.
       */}
      <span ref={rest} className="focus-rest" tabIndex={-1} />
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          className="menu"
          align="start"
          sideOffset={4}
          onKeyDown={() => (input.current = "keyboard")}
          onInteractOutside={() => (outside.current = true)}
          onCloseAutoFocus={(event) => {
            const clickedOutside = outside.current;
            outside.current = false;
            // Radix already leaves focus where an outside click put it, and its own bookkeeping
            // for that only resets if the event is left alone.
            if (clickedOutside) return;
            // A dialog opened from the menu takes focus itself; returning it to the trigger
            // first would steal it back.
            if (document.querySelector('[role="dialog"]')) {
              event.preventDefault();
              return;
            }
            if (input.current === "pointer") {
              event.preventDefault();
              rest.current?.focus();
            }
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
