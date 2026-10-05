import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { App } from "./app";

// Mounts the whole app on a memory router, so tests can read the address and how it changed
// (push or replace) without a browser.
export function mountApp(route: string) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const router = createMemoryRouter([{ path: "*", element: <App /> }], {
    initialEntries: [route],
  });
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { router, user: userEvent.setup() };
}

// The address as the reader would see it, e.g. "/?date=2025-09-30".
export function address(router: ReturnType<typeof mountApp>["router"]) {
  const { pathname, search } = router.state.location;
  return pathname + search;
}
