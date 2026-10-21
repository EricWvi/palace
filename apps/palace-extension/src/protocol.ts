// What the content script, the popup and the extension page ask of the service worker, and what
// it answers. Only the service worker holds the access token and talks to Palace; the others
// send these messages (see specs/decisions/extension/0-web-page-annotations-bound-and-migrated-
// into-articles.md, D8).
import type { components } from "../../palace-web/src/lib/generated/api";

type Schemas = components["schemas"];
export type AnnotationStyle = Schemas["AnnotationStyle"];
export type Annotation = Schemas["Annotation"];
export type WebAnnotation = Schemas["WebAnnotation"];
export type WebAnnotationCreate = Schemas["WebAnnotationCreate"];
export type WebAnnotationUpdate = Schemas["WebAnnotationUpdate"];
export type WebPage = Schemas["WebPage"];
export type WebPageList = Schemas["WebPageList"];
export type ArticlePage = Schemas["ArticlePage"];

// The article a bound page shows, with what the read-only display needs to place its marks.
export type BoundArticle = {
  id: string;
  title: string;
  // The article's page in Palace.
  link: string;
  // UTF-16 length of the Markdown body: annotation offsets over it give a rough position.
  length: number;
};

// What a page is to the extension: annotated on its own, or handed to an article.
export type PageState =
  | { kind: "unbound"; annotations: WebAnnotation[] }
  | { kind: "bound"; article: BoundArticle; annotations: Annotation[] };

// The address and whether a token is stored; the token itself never leaves the service worker.
export type SettingsView = { server: string; configured: boolean };

// Why a request failed, in the terms the pages act on.
export type Problem =
  "unconfigured" | "offline" | "unauthorized" | "bound" | "missing" | "invalid";

export type Reply<T> =
  { ok: true; data: T } | { ok: false; problem: Problem; message: string };

// Each request with the data its reply carries.
type Exchanges = {
  page: [{ url: string }, PageState];
  create: [{ annotation: WebAnnotationCreate }, WebAnnotation];
  update: [{ id: string; change: WebAnnotationUpdate }, WebAnnotation];
  delete: [{ id: string }, null];
  candidates: [{ near: string; q: string; cursor: string | null }, ArticlePage];
  bind: [{ url: string; title: string; article_id: string }, PageState];
  pages: [object, WebPageList];
  settings: [object, SettingsView];
  "save-settings": [{ server: string; token: string }, SettingsView];
};

export type Kind = keyof Exchanges;
export type Request<K extends Kind = Kind> = K extends Kind
  ? { kind: K } & Exchanges[K][0]
  : never;
export type Answer<K extends Kind> = Exchanges[K][1];

// What a content script may ask: it runs inside any web page, so it can read and change the
// annotations of the page it is on and nothing else of the owner's.
export const contentKinds: ReadonlySet<Kind> = new Set<Kind>([
  "page",
  "create",
  "update",
  "delete",
]);

// Sends a request to the service worker and waits for its reply.
export function send<K extends Kind>(
  request: Request<K>,
): Promise<Reply<Answer<K>>> {
  return chrome.runtime.sendMessage(request);
}

// The address of a page as an annotation's page: everything but the fragment.
export function pageAddress(href: string) {
  const hash = href.indexOf("#");
  return hash === -1 ? href : href.slice(0, hash);
}
