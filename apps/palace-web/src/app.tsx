import { lazy, Suspense, useEffect } from "react";
import { Link, Route, Routes, useLocation } from "react-router-dom";
import { PalaceHeader, type Section } from "./components/palace-header";
import { ShortcutHelp } from "./components/shortcut-help";
import { TimelinePage } from "./pages/timeline";
const StarsPage = lazy(() =>
  import("./pages/stars").then((module) => ({ default: module.StarsPage })),
);
const NotePage = lazy(() =>
  import("./pages/note").then((module) => ({ default: module.NotePage })),
);
const ArticlePage = lazy(() =>
  import("./pages/article").then((module) => ({ default: module.ArticlePage })),
);
const EchoPage = lazy(() =>
  import("./pages/echo").then((module) => ({ default: module.EchoPage })),
);
const TodoPage = lazy(() =>
  import("./pages/todo").then((module) => ({ default: module.TodoPage })),
);
const SleepPage = lazy(() =>
  import("./pages/sleep").then((module) => ({ default: module.SleepPage })),
);
const HabitsPage = lazy(() =>
  import("./pages/habits").then((module) => ({ default: module.HabitsPage })),
);
const ConversationPage = lazy(() =>
  import("./pages/conversation").then((module) => ({
    default: module.ConversationPage,
  })),
);

// Each page belongs to one section of the top bar; a day, its sleep and its 日课 in 时刻, tasks in 行事, notes, articles and
// conversations are listed and read in 摘星, jottings in 回响.
function sectionOf(pathname: string): Section | undefined {
  if (pathname === "/" || pathname === "/sleep" || pathname === "/habits")
    return "时刻";
  if (/^\/todos(\/|$)/.test(pathname)) return "行事";
  if (/^\/(notes|articles|conversations)(\/|$)/.test(pathname)) return "摘星";
  if (pathname === "/jottings") return "回响";
  return undefined;
}

const sheetClass: Record<Section, string> = {
  时刻: "sheet",
  行事: "sheet roomy",
  旅途: "sheet",
  摘星: "sheet",
  回响: "sheet wide",
};

export function App() {
  const { pathname } = useLocation();
  const section = sectionOf(pathname);
  // A reading page names the tab after what it shows; every other page names its section, so
  // tabs open on different sections tell apart.
  useEffect(() => {
    if (!/^\/(notes|articles|conversations)\//.test(pathname))
      document.title = section ? `Palace · ${section}` : "Palace";
  }, [pathname, section]);
  return (
    <ShortcutHelp>
      {/*
       * 回响 needs room for its columns and 行事 for its list column, so their sheets widen and the
       * header stretches with them.
       */}
      <main className={sheetClass[section ?? "时刻"]}>
        <PalaceHeader active={section} />
        <Suspense fallback={null}>
          <Routes>
            <Route path="/" element={<TimelinePage />} />
            <Route path="/sleep" element={<SleepPage />} />
            <Route path="/habits" element={<HabitsPage />} />
            <Route path="/notes" element={<StarsPage kind="notes" />} />
            <Route path="/notes/*" element={<NotePage />} />
            <Route path="/articles" element={<StarsPage kind="articles" />} />
            <Route path="/articles/:id" element={<ArticlePage />} />
            <Route
              path="/conversations"
              element={<StarsPage kind="conversations" />}
            />
            <Route path="/conversations/:id" element={<ConversationPage />} />
            <Route path="/jottings" element={<EchoPage />} />
            <Route path="/todos" element={<TodoPage view="today" />} />
            <Route path="/todos/inbox" element={<TodoPage view="inbox" />} />
            <Route path="/todos/lists/:id" element={<TodoPage view="list" />} />
            <Route
              path="*"
              element={
                <p className="quiet-note">
                  这里什么也没有。<Link to="/">回到今天</Link>
                </p>
              }
            />
          </Routes>
        </Suspense>
      </main>
    </ShortcutHelp>
  );
}
