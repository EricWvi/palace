import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "./ui/dialog";

// One line of the panel: the keys that do the same thing, and what they do.
export type Shortcut = { keys: string[]; action: string };
export type ShortcutGroup = { title: string; shortcuts: Shortcut[] };

// The modifier the platform's own shortcuts use, as its keyboards print it.
export const modKey = /Mac|iPhone|iPad/.test(navigator.userAgent)
  ? "⌘"
  : "Ctrl";

const general: ShortcutGroup = {
  title: "通用",
  shortcuts: [{ keys: ["?"], action: "显示或关闭快捷键" }],
};

type Register = (group: ShortcutGroup) => () => void;
const Registry = createContext<Register>(() => () => {});

// Lists a group in the `?` panel while the calling component is mounted. Each component that
// handles keys describes them next to its handler, so the panel always matches what the page
// really does. Pass a module-level constant: a new object every render would register anew.
export function useShortcutHelp(group: ShortcutGroup) {
  const register = useContext(Registry);
  useEffect(() => register(group), [register, group]);
}

const typing = (target: EventTarget | null) =>
  target instanceof Element &&
  target.closest("input, textarea, select, [contenteditable='true']") !== null;

// Wraps the app: `?` outside a text field toggles the panel of the shortcuts the mounted
// components registered, page groups first and the always-present keys last.
export function ShortcutHelp({ children }: { children: ReactNode }) {
  // A plain set, not state: the panel snapshots it on opening, so registering never re-renders.
  const registered = useRef(new Set<ShortcutGroup>());
  const [shown, setShown] = useState<ShortcutGroup[] | null>(null);
  const register = useCallback<Register>((group) => {
    registered.current.add(group);
    return () => registered.current.delete(group);
  }, []);

  useEffect(() => {
    function toggle(event: KeyboardEvent) {
      if (event.key !== "?" || event.metaKey || event.ctrlKey || event.altKey)
        return;
      if (typing(event.target)) return;
      if (shown) {
        event.preventDefault();
        setShown(null);
        return;
      }
      // Another dialog or an open menu owns the keyboard.
      if (document.querySelector('[role="dialog"], [role="menu"]')) return;
      event.preventDefault();
      setShown([...registered.current, general]);
    }
    document.addEventListener("keydown", toggle);
    return () => document.removeEventListener("keydown", toggle);
  }, [shown]);

  return (
    <Registry value={register}>
      {children}
      <Dialog
        open={shown !== null}
        onOpenChange={(open) => {
          if (!open) setShown(null);
        }}
      >
        <DialogContent
          className="shortcut-help"
          aria-describedby={undefined}
          // Nothing in the panel takes input, so no control should light up on opening.
          onOpenAutoFocus={(event) => event.preventDefault()}
        >
          <DialogHeader>
            <DialogTitle>快捷键</DialogTitle>
          </DialogHeader>
          {shown?.map((group) => (
            <section key={group.title}>
              <h3>{group.title}</h3>
              <dl>
                {group.shortcuts.map((shortcut) => (
                  <div key={shortcut.action}>
                    <dt>
                      {shortcut.keys.map((key) => (
                        <kbd key={key}>{key}</kbd>
                      ))}
                    </dt>
                    <dd>{shortcut.action}</dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </DialogContent>
      </Dialog>
    </Registry>
  );
}
