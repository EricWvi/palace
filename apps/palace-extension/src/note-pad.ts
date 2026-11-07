// The 总批 card on a web page: one block of text pinned at the corner of the window, there as long
// as annotating is on so a thought has somewhere to go. It belongs to the extension's closed
// shadow root. Saving happens as typing pauses; what could not be saved stays in the card, and the
// service worker keeps it on this computer until Palace has it.
import type { OverallNote, PageNote, Reply } from "./protocol";

// How long typing must pause before what was written is saved.
const SAVE_AFTER_MS = 800;

export const padCss = `
.pad {
  position: fixed; right: 24px; bottom: 24px; z-index: 2147483645; width: 300px;
  background: var(--surface-raised); border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: 0 2px 6px rgb(0 0 0 / 0.07), 0 12px 32px rgb(0 0 0 / 0.13);
  color: var(--text-2); font-family: var(--font-ui); font-size: var(--font-size-sm); line-height: 1.65;
}
.pad.open { padding: 10px 14px 12px; color: var(--text); }
.pad-fold {
  display: flex; gap: 10px; align-items: baseline; box-sizing: border-box; width: 100%;
  padding: 7px 14px;
}
.pad-name { flex: none; color: var(--text-3); font-family: var(--font-display); letter-spacing: 0.08em; }
.pad-first { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.pad textarea {
  all: unset; display: block; box-sizing: border-box; width: 100%; min-height: 6.6em; max-height: 40vh;
  margin-top: 4px; overflow-y: auto; white-space: pre-wrap; overflow-wrap: anywhere; field-sizing: content;
}
.pad-problem { margin-top: 6px; color: var(--accent); }
`;

// Saves the text over `version`; the reply says what Palace now holds, or why it refused.
export type SaveNote = (
  text: string,
  version: number,
) => Promise<Reply<OverallNote>>;

export class NotePad {
  private card: HTMLElement | null = null;
  private field: HTMLTextAreaElement | null = null;
  private text = "";
  private version = 0;
  private problem: string | null = null;
  private pause = 0;
  private dirty = false;
  // Saves run one at a time, each naming the version the last one produced.
  private queue: Promise<boolean> = Promise.resolve(true);

  constructor(
    private readonly root: ShadowRoot,
    private readonly mount: () => void,
    private readonly save: SaveNote,
  ) {}

  // Shows a page's note folded; a draft Palace does not have yet opens, so it is seen.
  show(note: PageNote) {
    this.text = note.text;
    this.version = note.version;
    this.problem = note.unsaved ? "这段总批还没有保存到 Palace。" : null;
    this.dirty = note.unsaved;
    if (note.unsaved) this.write();
    else this.fold();
  }

  // Takes the card away, handing over anything not saved yet.
  hide() {
    void this.flush();
    this.field = null;
    this.card?.remove();
    this.card = null;
  }

  // Opens the card for writing, wherever it was.
  write() {
    this.render((card) => {
      card.className = "pad open";
      const name = document.createElement("span");
      name.className = "pad-name";
      name.textContent = "总批";
      const field = document.createElement("textarea");
      field.value = this.text;
      field.setAttribute("aria-label", "总批");
      field.addEventListener("input", () => {
        this.text = field.value;
        this.dirty = true;
        clearTimeout(this.pause);
        this.pause = window.setTimeout(() => void this.flush(), SAVE_AFTER_MS);
      });
      field.addEventListener("blur", () => void this.finish());
      field.addEventListener("keydown", (event) => {
        if (
          event.key === "Escape" ||
          (event.key === "Enter" && (event.metaKey || event.ctrlKey))
        ) {
          event.preventDefault();
          field.blur();
        }
      });
      card.append(name, field);
      if (this.problem) card.append(this.problemLine(this.problem));
      this.field = field;
    });
    this.field?.focus();
    this.field?.setSelectionRange(this.text.length, this.text.length);
  }

  // Sends what is unsaved; resolves to whether Palace now has all of it.
  flush(): Promise<boolean> {
    clearTimeout(this.pause);
    this.queue = this.queue.then(async () => {
      if (!this.dirty) return true;
      const text = this.text;
      const reply = await this.save(text, this.version);
      if (!reply.ok) {
        this.say(reply.message);
        return false;
      }
      this.version = reply.data.version;
      // Words typed while this save was on its way still wait for theirs.
      this.dirty = this.text !== text;
      this.say(null);
      return true;
    });
    return this.queue;
  }

  // The card folds only once Palace has the text, so a failed save stays on screen.
  private async finish() {
    if (await this.flush()) this.fold();
  }

  private fold() {
    this.field = null;
    this.render((card) => {
      card.className = "pad";
      const fold = document.createElement("button");
      fold.className = "pad-fold";
      fold.setAttribute("aria-label", "总批");
      const name = document.createElement("span");
      name.className = "pad-name";
      name.textContent = "总批";
      const first = document.createElement("span");
      first.className = "pad-first";
      first.textContent = this.text.split("\n")[0];
      fold.append(name, first);
      fold.addEventListener("click", () => this.write());
      card.append(fold);
    });
  }

  private problemLine(text: string) {
    const line = document.createElement("p");
    line.className = "pad-problem";
    line.setAttribute("role", "alert");
    line.textContent = text;
    return line;
  }

  private say(problem: string | null) {
    this.problem = problem;
    this.card?.querySelector(".pad-problem")?.remove();
    if (problem && this.field) this.card?.append(this.problemLine(problem));
  }

  // Fills the card, creating it on first use. Keys typed in it stay away from the page, whose
  // own shortcuts must not fire on them.
  private render(fill: (card: HTMLElement) => void) {
    if (!this.card) {
      this.mount();
      this.card = document.createElement("aside");
      this.card.setAttribute("aria-label", "总批");
      for (const type of ["keydown", "keypress", "keyup"])
        this.card.addEventListener(type, (event) => event.stopPropagation());
      this.root.append(this.card);
    }
    this.card.replaceChildren();
    fill(this.card);
  }
}
