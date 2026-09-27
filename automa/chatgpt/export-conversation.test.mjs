import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { JSDOM } from "jsdom";

const workflow = JSON.parse(
  readFileSync(
    new URL("./export-conversation.automa.json", import.meta.url),
    "utf8",
  ),
);
const nodes = new Map(workflow.drawflow.nodes.map((node) => [node.id, node]));
const loop = nodes.get("loopturn").data;
const loopId = "chatgpt-test-loop";

// Automa 1.30's generateLoopSelectors turns each marked element into this selector.
const withLoopData = (selector, index) =>
  selector.replaceAll(
    `{{loopData@${loop.loopId}}}`,
    `[automa-loop="${loopId}--${index}"]`,
  );
const existsSelector = (nodeId) =>
  nodes.get(nodeId).data.conditions[0].conditions[0].conditions[0].items[0].data
    .selector;
const nextHandle = (source, output) =>
  nodes.get(
    workflow.drawflow.edges.find(
      (edge) => edge.sourceHandle === `${source}-output-${output}`,
    ).target,
  ).id;

// Mirrors the ChatGPT DOM observed in September 2026: one data-turn-key per
// question/answer pair, with the assistant copy button in the turn action bar
// and extra "复制" buttons inside code block toolbars.
function turn(key, { user = true, assistant = true } = {}) {
  return `
    <div data-turn-key="${key}">
      <div data-content-search-turn-key="fallback-turn-0">
        <div class="group flex flex-col pb-2 pt-2">
          <div class="flex flex-col gap-3">
            ${
              user
                ? `<div data-chatgpt-search-unit-key="fallback-turn-0:0:user">
                     <div class="turn-action-controls">
                       <button aria-label="复制消息"></button>
                     </div>
                   </div>`
                : ""
            }
            <div data-chatgpt-search-unit-key="fallback-turn-0:2:assistant">
              <div data-markdown-copy="code-block">
                <div data-markdown-copy="exclude"><button aria-label="复制"></button></div>
              </div>
            </div>
          </div>
          ${
            assistant
              ? `<div class="turn-action-controls"><button aria-label="复制"></button></div>`
              : ""
          }
        </div>
      </div>
    </div>`;
}

function markProcessed(document) {
  document.querySelectorAll(loop.selector).forEach((element, index) => {
    element.setAttribute("automa-loop", `${loopId}--${index}`);
  });
}

// Runs a background JavaScript node against an in-memory variable store.
function runBackground(nodeId, variables) {
  const store = structuredClone(variables);
  new Function(
    "automaRefData",
    "automaSetVariable",
    "console",
    nodes.get(nodeId).data.code,
  )(
    (_, name) => store[name],
    (name, value) => {
      store[name] = value;
    },
    { log() {} },
  );
  return store;
}

// Runs a role node followed by the append node, as the copy branch does.
function append(store, roleNode, copiedText) {
  return runBackground("b0vvwkb", {
    ...runBackground(roleNode, store),
    currentCopiedText: copiedText,
  });
}

function startTurn(store, id) {
  return runBackground("4xsplhe", { ...store, currentTurnId: id });
}

test("no node executes JavaScript inside the ChatGPT page", () => {
  const pageScripts = workflow.drawflow.nodes.filter(
    (node) =>
      node.label === "javascript-code" && node.data.context !== "background",
  );
  assert.deepEqual(pageScripts, []);
});

test("next batch excludes processed turns and finds newly mounted ones", () => {
  const dom = new JSDOM(turn("t1") + turn("t2"));
  try {
    const { document } = dom.window;
    markProcessed(document);
    const next = `${loop.selector}:not([automa-loop*="${loopId}"])`;
    assert.deepEqual([...document.querySelectorAll(next)], []);

    document.body.insertAdjacentHTML("beforeend", turn("t3"));
    assert.deepEqual(
      [...document.querySelectorAll(next)].map(
        (element) => element.dataset.turnKey,
      ),
      ["t3"],
    );
  } finally {
    dom.window.close();
  }
});

test("copy selectors pick the turn buttons and skip code block buttons", () => {
  const dom = new JSDOM(
    turn("t1") + turn("t2", { user: false, assistant: false }),
  );
  try {
    const { document } = dom.window;
    markProcessed(document);
    const matches = (selector, index) =>
      [...document.querySelectorAll(withLoopData(selector, index))].map(
        (button) => button.parentElement.className,
      );
    assert.deepEqual(
      {
        user: matches(existsSelector("hasuser"), 0),
        assistant: matches(existsSelector("hasasst"), 0),
        emptyTurn: [
          ...matches(existsSelector("hasuser"), 1),
          ...matches(existsSelector("hasasst"), 1),
        ],
        clicksMatchConditions: [
          nodes.get("clickuser").data.selector === existsSelector("hasuser"),
          nodes.get("clickasst").data.selector === existsSelector("hasasst"),
        ],
      },
      {
        user: ["turn-action-controls"],
        assistant: ["turn-action-controls"],
        emptyTurn: [],
        clicksMatchConditions: [true, true],
      },
    );
  } finally {
    dom.window.close();
  }
});

test("a turn exports user then assistant and then reaches the breakpoint", () => {
  let store = startTurn(runBackground("jiglnit", {}), "t1");
  store = append(store, "setuser", "问题");
  const afterUser = store.currentMessageRole;
  store = append(store, "setasst", "回答");
  assert.deepEqual(
    {
      conversation: store.conversation,
      seenTurnIds: store.seenTurnIds,
      turnMessageCount: store.turnMessageCount,
      afterUser,
      userNext: nextHandle("afteruser", "chatgptAfterUser"),
      assistantNext: nextHandle("afteruser", "fallback"),
    },
    {
      conversation: [
        { role: "user", content: "问题", turnId: "t1" },
        { role: "assistant", content: "回答", turnId: "t1" },
      ],
      seenTurnIds: ["t1"],
      turnMessageCount: 2,
      afterUser: "user",
      userNext: "hasasst",
      assistantNext: "cqwhcdl",
    },
  );
});

test("the clipboard placeholder is rejected instead of being exported", () => {
  const store = startTurn(runBackground("jiglnit", {}), "t1");
  assert.throws(
    () => append(store, "setuser", nodes.get("clruser").data.dataToCopy),
    /没有从剪贴板读到新的复制内容/,
  );
});

test("a turn without any copy button stops the workflow", () => {
  const store = startTurn(runBackground("jiglnit", {}), "t1");
  assert.deepEqual(nextHandle("hasasst", "fallback"), "turncheck");
  assert.throws(
    () => runBackground("turncheck", store),
    /没有找到 user 或 assistant 的复制按钮/,
  );
});

test("a remounted exported turn goes straight to the loop breakpoint", () => {
  const first = startTurn(runBackground("jiglnit", {}), "t1");
  const again = startTurn(first, "t1");
  assert.deepEqual(
    {
      skip: again.currentTurnSkip,
      seenTurnIds: again.seenTurnIds,
      next: nextHandle("yzpfobl", "chatgptTurnSeen"),
    },
    { skip: "yes", seenTurnIds: ["t1"], next: "cqwhcdl" },
  );
});

test("the workflow scrolls the reversed thread to the top before looping", () => {
  const scroll = nodes.get("scrolltop").data;
  assert.deepEqual(
    {
      afterInit: nextHandle("jiglnit", "1"),
      beforeLoop: nextHandle(nextHandle("scrolltop", "1"), "1"),
      selector: scroll.selector,
      scrollIntoView: scroll.scrollIntoView,
      scrollsUpPastAnyHeight: scroll.scrollY <= -1e6,
    },
    {
      afterInit: "scrolltop",
      beforeLoop: "loopturn",
      selector: ".thread-scroll-container",
      scrollIntoView: false,
      scrollsUpPastAnyHeight: true,
    },
  );
});
