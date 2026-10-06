import { Link } from "react-router-dom";
import { Ornament } from "./ornament";

export type Section = "时刻" | "行事" | "旅途" | "摘星" | "回响";
const sections: Section[] = ["时刻", "行事", "旅途", "摘星", "回响"];

// Only 时刻 has a page of its own so far. The others stay visible as plain words, so the bar
// keeps its shape as they arrive, but they are not links and never take keyboard focus.
export function PalaceHeader({ active }: { active?: Section }) {
  return (
    <header className="top">
      <Ornament />
      <nav aria-label="主导航">
        {sections.map((section) => {
          const current = section === active ? "page" : undefined;
          return section === "时刻" ? (
            <Link
              key={section}
              to="/"
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
