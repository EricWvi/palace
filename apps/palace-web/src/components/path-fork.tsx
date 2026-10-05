import { branchChoices, type ResolvedPath } from "@/lib/conversation-tree";

// A fork is a sentence, not a control: the options are the first words of each branch, and
// picking one swaps everything below it.
export function PathFork({
  paths,
  selected,
  after,
  onSelect,
}: {
  paths: ResolvedPath[];
  selected: ResolvedPath;
  after: number;
  onSelect: (pathId: string) => void;
}) {
  const choices = branchChoices(paths, selected, after);
  if (choices.length < 2) return null;
  const next = selected.messages[after + 1];
  const current = next ? `message:${next.id}` : `path:${selected.path.id}`;
  return (
    <li className="fork">
      <div role="group" aria-label={`第 ${after + 1} 条消息后的分支`}>
        此处分为 {choices.length} 支：
        <span className="options">
          {choices.map((choice) => (
            <button
              key={choice.key}
              type="button"
              aria-current={choice.key === current ? "true" : undefined}
              onClick={() => {
                if (choice.key !== current) onSelect(choice.pathId);
              }}
            >
              {choice.label}
            </button>
          ))}
        </span>
      </div>
    </li>
  );
}
