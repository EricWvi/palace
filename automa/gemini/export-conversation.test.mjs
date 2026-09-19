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
const selector = workflow.drawflow.nodes.find((node) => node.id === "zzmi7il")
  .data.selector;
const loopId = "gemini-test-loop";

// Automa 1.30's excludeSelector appends this suffix to the entire selector string.
const nextBatchSelector = `${selector}:not([automa-loop*="${loopId}"])`;

function page() {
  return new JSDOM(
    '<user-query id="u1"></user-query><model-response id="a1"></model-response><user-query id="u2"></user-query><model-response id="a2"></model-response>',
  );
}

function markProcessed(document) {
  document.querySelectorAll(selector).forEach((element, index) => {
    element.setAttribute("automa-loop", `${loopId}--${index}`);
  });
}

test("first pass includes both roles in document order", () => {
  const dom = page();
  try {
    assert.deepEqual(
      [...dom.window.document.querySelectorAll(selector)].map(
        (element) => element.id,
      ),
      ["u1", "a1", "u2", "a2"],
    );
  } finally {
    dom.window.close();
  }
});

test("after the complete first pass there is no user-only second batch", () => {
  const dom = page();
  try {
    markProcessed(dom.window.document);
    assert.deepEqual(
      [...dom.window.document.querySelectorAll(nextBatchSelector)].map(
        (element) => element.id,
      ),
      [],
    );
  } finally {
    dom.window.close();
  }
});

test("loading more selects only the newly mounted messages of both roles", () => {
  const dom = page();
  try {
    const { document } = dom.window;
    markProcessed(document);
    for (const [tag, id] of [
      ["user-query", "u3"],
      ["model-response", "a3"],
    ]) {
      const element = document.createElement(tag);
      element.id = id;
      document.body.append(element);
    }
    assert.deepEqual(
      [...document.querySelectorAll(nextBatchSelector)].map(
        (element) => element.id,
      ),
      ["u3", "a3"],
    );
  } finally {
    dom.window.close();
  }
});

test("confirmed replay reaches Export without the element breakpoint", () => {
  const edge = workflow.drawflow.edges.find(
    (item) => item.sourceHandle === "loopdone-output-geminiReplayDone",
  );
  const node = workflow.drawflow.nodes.find((item) => item.id === edge.target);
  assert.equal(node.label, "export-data");
});
