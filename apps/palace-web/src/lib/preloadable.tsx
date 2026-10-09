import { lazy, useState, type ComponentType } from "react";

// A lazily loaded component whose chunk can be fetched ahead of use. React.lazy suspends on its
// first render even when the chunk has long arrived, and React then holds the reveal for its
// Suspense throttle (about 300ms): a dialog prefetched on approach still opened half a second
// late, once per page load. Once preloaded, this renders the loaded component directly and never
// suspends; only a render that beats the fetch goes through React.lazy.
export function preloadable<P extends object>(
  load: () => Promise<ComponentType<P>>,
) {
  let loaded: ComponentType<P> | undefined;
  let pending: Promise<ComponentType<P>> | undefined;
  function preload() {
    pending ??= load().then(
      (component) => (loaded = component),
      (error: unknown) => {
        // A failed fetch must not stick: the next approach or render tries again.
        pending = undefined;
        throw error;
      },
    );
    return pending;
  }
  const Lazy = lazy(() =>
    preload().then((component) => ({ default: component })),
  );
  function Preloadable(props: P) {
    // Chosen once per mount. Switching to the loaded type after the lazy one resolved would be a
    // different component to React, remounting it and dropping what the reader had typed.
    const [Component] = useState<ComponentType<P>>(() => loaded ?? Lazy);
    return <Component {...props} />;
  }
  return { Component: Preloadable, preload };
}
