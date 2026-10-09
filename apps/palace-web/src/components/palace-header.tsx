import { Link } from "react-router-dom";
import { Ornament } from "./ornament";

export type Section = "时刻" | "行事" | "旅途" | "摘星" | "回响";
const sections: Section[] = ["时刻", "行事", "旅途", "摘星", "回响"];
const pages: Partial<Record<Section, string>> = {
  时刻: "/",
  摘星: "/conversations",
};

// Sections with a page are links: 时刻 opens today, 摘星 its conversation list. The others stay
// visible as plain words, so the bar keeps its shape as they arrive, but they are not links and
// never take keyboard focus.
export function PalaceHeader({ active }: { active?: Section }) {
  return (
    <header className="top">
      <Ornament />
      <nav aria-label="主导航">
        {sections.map((section) => {
          const current = section === active ? "page" : undefined;
          const page = pages[section];
          return page ? (
            <Link
              key={section}
              to={page}
              aria-current={current}
              // The bar outlives every page, so a link Chrome focused on click would keep focus
              // after navigating, and the next shortcut key (← → stepping days) would ring it as
              // :focus-visible. A click therefore leaves focus alone, as Safari does natively;
              // Tab still reaches the link.
              onMouseDown={(event) => event.preventDefault()}
            >
              {section}
            </Link>
          ) : (
            <span key={section} aria-current={current}>
              {section}
            </span>
          );
        })}
      </nav>
    </header>
  );
}
