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
const ConversationPage = lazy(() =>
  import("./pages/conversation").then((module) => ({
    default: module.ConversationPage,
  })),
);

// Each page belongs to one section of the top bar; notes and conversations are listed and read
// in 摘星.
function sectionOf(pathname: string): Section | undefined {
  if (pathname === "/") return "时刻";
  if (/^\/(notes|conversations)(\/|$)/.test(pathname)) return "摘星";
  return undefined;
}

export function App() {
  const { pathname } = useLocation();
  // A reading page names the tab after what it shows; every other page is just Palace.
  useEffect(() => {
    if (!/^\/(notes|conversations)\//.test(pathname)) document.title = "Palace";
  }, [pathname]);
  return (
    <main className="sheet">
      <PalaceHeader active={sectionOf(pathname)} />
      <Suspense fallback={null}>
        <Routes>
          <Route path="/" element={<TimelinePage />} />
          <Route path="/notes" element={<StarsPage kind="notes" />} />
          <Route path="/notes/*" element={<NotePage />} />
          <Route
            path="/conversations"
            element={<StarsPage kind="conversations" />}
          />
          <Route path="/conversations/:id" element={<ConversationPage />} />
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
