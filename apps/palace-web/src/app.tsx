import { lazy, Suspense, useEffect } from "react";
import { Link, Route, Routes, useLocation } from "react-router-dom";
import { PalaceHeader, type Section } from "./components/palace-header";
import { TimelinePage } from "./pages/timeline";
const ConversationPage = lazy(() =>
  import("./pages/conversation").then((module) => ({
    default: module.ConversationPage,
  })),
);

// Each page belongs to one section of the top bar; a conversation is read in 摘星.
function sectionOf(pathname: string): Section | undefined {
  if (pathname === "/") return "时刻";
  if (pathname.startsWith("/conversations/")) return "摘星";
  return undefined;
}

export function App() {
  const { pathname } = useLocation();
  // The reading page names the tab after the conversation; every other page is just Palace.
  useEffect(() => {
    if (!pathname.startsWith("/conversations/")) document.title = "Palace";
  }, [pathname]);
  return (
    <main className="sheet">
      <PalaceHeader active={sectionOf(pathname)} />
      <Suspense fallback={null}>
        <Routes>
          <Route path="/" element={<TimelinePage />} />
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
