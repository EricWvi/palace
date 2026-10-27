import { expect, it } from "vitest";
import { dollarMath } from "./math-delimiters";

// Core test case: `specs/test-cases/server/import/delta-update.md#written-messages-must-append-as-a-delta-and-reset-on-reopening`
it("turns either ChatGPT formula syntax into dollar delimiters, leaving code alone", () => {
  const legacy = ["复杂度为 $O(n^2)$。", "", "$$", "O(n^2)", "$$"].join("\n");
  const current = [
    "复杂度为 \\\\(O(n^2)\\\\)。",
    "",
    "\\\\[ O(n^2) \\\\]",
    "",
    "\\\\[ dp[i]=\\bigvee\\_{0\\le j\\<i}(dp[j]\\land(s[j:i]\\in wordDict)) \\\\]",
    "",
    "- 列表里：",
    "  \\\\[",
    "  a \\\\\\\\",
    "  b",
    "  \\\\]",
    "> 引用里 \\\\[x\\\\] 行内",
    "转义括号 \\(x\\) 不是公式",
    "",
    "代码 `\\\\(a\\\\)` 不变",
    "```tex",
    "\\\\[ y \\\\]",
    "```",
  ].join("\n");
  expect([legacy, current].map(dollarMath)).toEqual([
    legacy,
    [
      "复杂度为 $O(n^2)$。",
      "",
      "$$",
      "O(n^2)",
      "$$",
      "",
      "$$",
      "dp[i]=\\bigvee_{0\\le j<i}(dp[j]\\land(s[j:i]\\in wordDict))",
      "$$",
      "",
      "- 列表里：",
      "  $$",
      "  a \\\\",
      "  b",
      "  $$",
      "> 引用里 $$x$$ 行内",
      "转义括号 \\(x\\) 不是公式",
      "",
      "代码 `\\\\(a\\\\)` 不变",
      "```tex",
      "\\\\[ y \\\\]",
      "```",
    ].join("\n"),
  ]);
});
