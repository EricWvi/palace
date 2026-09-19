import { branchChoices, type ResolvedPath } from "@/lib/conversation-tree";

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
  const value = next ? `message:${next.id}` : `path:${selected.path.id}`;
  return (
    <label className="branch-picker path-fork">
      此处的对话分支
      <select
        className="select-input"
        aria-label={`第 ${after + 1} 条消息后的分支`}
        value={value}
        onChange={(event) => {
          const choice = choices.find(
            (item) => item.key === event.target.value,
          );
          if (choice) onSelect(choice.pathId);
        }}
      >
        {choices.map((choice) => (
          <option key={choice.key} value={choice.key}>
            {choice.label}
          </option>
        ))}
      </select>
    </label>
  );
}
