import assert from "node:assert/strict";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

// Relocate component references for AJV, including refs nested in inline response schemas.
export function responseValidator(document) {
  const ajv = new Ajv2020({ strict: true, allErrors: true });
  addFormats(ajv);
  for (const format of ["int32", "int64", "binary"])
    ajv.addFormat(format, true);
  const relocate = (value) =>
    JSON.parse(
      JSON.stringify(value, (key, child) =>
        key === "$ref" && typeof child === "string"
          ? child.replace(/^#\/components\/schemas\//, "#/$defs/")
          : child,
      ),
    );
  const definitions = relocate(document.components.schemas);
  const cache = new Map();
  return (sample) => {
    const label = `${sample.method} ${sample.path} ${sample.status}`;
    const operation =
      document.paths[sample.path]?.[sample.method.toLowerCase()];
    assert.ok(operation, `Undocumented operation: ${label}`);
    const response = operation.responses[String(sample.status)];
    assert.ok(response, `Undocumented status: ${label}`);
    assert.ok(
      !response.$ref,
      "Response references need explicit resolution before validation",
    );
    const media = (sample.headers["content-type"] ?? "")
      .split(";")[0]
      .trim()
      .toLowerCase();
    if (!response.content) {
      assert.equal(sample.body, "", `Unexpected response body: ${label}`);
      return;
    }
    const schema = response.content[media]?.schema;
    assert.ok(schema, `Undocumented media type ${media}: ${label}`);
    const key = `${label} ${media}`;
    if (!cache.has(key))
      cache.set(key, ajv.compile({ $defs: definitions, ...relocate(schema) }));
    const validate = cache.get(key);
    const value =
      media === "application/json" ? JSON.parse(sample.body) : sample.body;
    assert.ok(
      validate(value),
      `${label} ${media}: ${ajv.errorsText(validate.errors)}`,
    );
  };
}
