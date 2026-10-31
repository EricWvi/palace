// Fail explicitly if a test stops exercising the typed client's native Request transport.
export function testRequest(input: RequestInfo | URL): Request {
  if (!(input instanceof Request)) throw new Error("Expected a native Request");
  return input;
}
export function requestPath(input: RequestInfo | URL): string {
  return new URL(testRequest(input).url).pathname;
}
// Answers what 时刻 pins above a day — sleep and 日课 — as an owner who never slept and keeps
// no habits, for tests about something else on the day's page; anything else is left to the
// caller.
export function bareDay(path: string): Response | undefined {
  if (path === "/api/habits") return Response.json({ habits: [], checks: [] });
  if (path === "/api/sleep/nights") return Response.json([]);
  if (path === "/api/sleep/running") return Response.json({ started_at: null });
  return undefined;
}
