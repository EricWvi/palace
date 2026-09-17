import { Link, Route, Routes } from "react-router-dom";
import { Library, Sparkles } from "lucide-react";
import { LibraryPage } from "./pages/library";
import { lazy, Suspense } from "react";
const ConversationPage = lazy(() =>
  import("./pages/conversation").then((module) => ({
    default: module.ConversationPage,
  })),
);
export function App() {
  return (
    <div className="shell">
      <aside className="sidebar">
        <Link className="brand" to="/">
          <span className="brand-mark">
            <Sparkles size={22} />
          </span>
          palace<span className="brand-dot">.</span>
        </Link>
        <p className="eyebrow">你的思考，值得留存</p>
        <nav>
          <Link className="nav-link" to="/">
            <Library size={18} />
            会话收藏
          </Link>
        </nav>
        <div className="sidebar-footer">
          <span className="status-dot" /> 为灵感留一个位置
          <small>YOUR PERSONAL CONVERSATION LIBRARY</small>
        </div>
      </aside>
      <main className="main">
        <Suspense
          fallback={
            <p role="status" className="empty">
              正在加载页面…
            </p>
          }
        >
          <Routes>
            <Route path="/" element={<LibraryPage />} />
            <Route path="/conversations/:id" element={<ConversationPage />} />
            <Route
              path="*"
              element={
                <div className="empty">
                  <h1>页面不存在</h1>
                  <Link to="/">返回会话收藏</Link>
                </div>
              }
            />
          </Routes>
        </Suspense>
      </main>
    </div>
  );
}
