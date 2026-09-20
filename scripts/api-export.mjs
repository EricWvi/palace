import { execFileSync } from "node:child_process";

export function exportContract(root) {
  // Finish compilation and serialization before touching the committed contract.
  const output = execFileSync(
    "cargo",
    [
      "run",
      "--locked",
      "--quiet",
      "-p",
      "palace-backend",
      "--example",
      "export_openapi",
    ],
    { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
  );
  JSON.parse(output);
  return output;
}
