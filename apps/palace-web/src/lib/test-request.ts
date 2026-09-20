// Fail explicitly if a test stops exercising the typed client's native Request transport.
export function testRequest(input: RequestInfo | URL): Request {
  if (!(input instanceof Request)) throw new Error("Expected a native Request");
  return input;
}
export function requestPath(input: RequestInfo | URL): string {
  return new URL(testRequest(input).url).pathname;
}
