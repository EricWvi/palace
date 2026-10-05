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
            <Link key={section} to="/" aria-current={current}>
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
