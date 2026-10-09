import { Suspense, useState } from "react";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { preloadable } from "./preloadable";

// Counts how often a boundary had to show its fallback, i.e. how often the component suspended.
function boundary() {
  const shown: string[] = [];
  function Fallback() {
    shown.push("fallback");
    return null;
  }
  return { shown, Fallback };
}
function Greeting({ name }: { name: string }) {
  return <p>你好，{name}</p>;
}

it("renders a preloaded component without suspending, so React has no reveal to throttle", async () => {
  const greeting = preloadable(async () => Greeting);
  await greeting.preload();
  const { shown, Fallback } = boundary();
  render(
    <Suspense fallback={<Fallback />}>
      <greeting.Component name="Eric" />
    </Suspense>,
  );
  // Present in the very first commit: React.lazy would have suspended here despite the preload.
  expect(screen.getByText("你好，Eric")).toBeInTheDocument();
  expect(shown).toEqual([]);
});

it("suspends only when rendered before its chunk, and keeps state once it arrives", async () => {
  let resolve!: () => void;
  const arrived = new Promise<void>((r) => (resolve = r));
  function Counter() {
    const [count, setCount] = useState(0);
    return <button onClick={() => setCount(count + 1)}>{count}</button>;
  }
  const counter = preloadable(async () => {
    await arrived;
    return Counter;
  });
  const { shown, Fallback } = boundary();
  const view = render(
    <Suspense fallback={<Fallback />}>
      <counter.Component />
    </Suspense>,
  );
  expect(shown).toEqual(["fallback"]);
  await act(async () => resolve());
  const button = await screen.findByRole("button");
  await userEvent.click(button);
  // A rerender after the chunk arrived must not swap in the loaded type and remount.
  view.rerender(
    <Suspense fallback={<Fallback />}>
      <counter.Component />
    </Suspense>,
  );
  expect(screen.getByRole("button")).toHaveTextContent("1");
});

it("tries again after a failed fetch", async () => {
  let attempts = 0;
  const flaky = preloadable(async () => {
    attempts += 1;
    if (attempts === 1) throw new Error("offline");
    return Greeting;
  });
  await expect(flaky.preload()).rejects.toThrow("offline");
  await flaky.preload();
  expect(attempts).toBe(2);
});
