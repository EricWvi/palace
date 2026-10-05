import { File, Blob } from "node:buffer";
import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";
afterEach(cleanup);
// Lazily loaded pages and dialogs are compiled on first use, which can take longer than the
// default one second on a busy worker.
configure({ asyncUtilTimeout: 5000 });

// Use one native File/Blob/FormData family with Node's Request; jsdom mixes incompatible implementations.
// Obtain the matching native constructor without replacing the transport with a mock.
const nativeForm = await new Response("", {
  headers: { "Content-Type": "application/x-www-form-urlencoded" },
}).formData();
for (const [name, value] of Object.entries({
  File,
  Blob,
  FormData: nativeForm.constructor,
})) {
  Object.defineProperty(globalThis, name, {
    value,
    configurable: true,
    writable: true,
  });
}
