import { lazy, Suspense, useEffect } from "react";
import { Link, Route, Routes, useLocation } from "react-router-dom";
import { PalaceHeader, type Section } from "./components/palace-header";
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
const ConversationPage = lazy(() =>
  import("./pages/conversation").then((module) => ({
    default: module.ConversationPage,
  })),
);

// Each page belongs to one section of the top bar; notes, articles and conversations are listed
// and read in 摘星, jottings in 回响.
function sectionOf(pathname: string): Section | undefined {
  if (pathname === "/") return "时刻";
  if (/^\/(notes|articles|conversations)(\/|$)/.test(pathname)) return "摘星";
  if (pathname === "/jottings") return "回响";
  return undefined;
}

export function App() {
  const { pathname } = useLocation();
  const section = sectionOf(pathname);
  // A reading page names the tab after what it shows; every other page is just Palace.
  useEffect(() => {
    if (!/^\/(notes|articles|conversations)\//.test(pathname))
      document.title = "Palace";
  }, [pathname]);
  return (
    // 回响 needs room for its columns, so its sheet widens and the header stretches with it.
    <main className={section === "回响" ? "sheet wide" : "sheet"}>
      <PalaceHeader active={section} />
      <Suspense fallback={null}>
        <Routes>
          <Route path="/" element={<TimelinePage />} />
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
  );
}
