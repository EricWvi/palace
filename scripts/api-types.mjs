import openapiTS, { astToString, COMMENT_HEADER } from "openapi-typescript";
import { format } from "prettier";
import ts from "typescript";

// Generate in memory so checks never overwrite local artifacts.
export async function browserTypes(contract) {
  const document = structuredClone(contract);
  // Origin is mandatory on the wire but controlled by the browser, not by application code.
  for (const item of Object.values(document.paths)) {
    for (const operation of Object.values(item)) {
      if (
        operation &&
        typeof operation === "object" &&
        Array.isArray(operation.parameters)
      ) {
        operation.parameters = operation.parameters.filter(
          (parameter) =>
            !(
              parameter.in === "header" &&
              parameter.name.toLowerCase() === "origin"
            ),
        );
      }
    }
  }
  const nodes = await openapiTS(document, {
    transform(schema) {
      if (schema.format === "binary")
        return ts.factory.createTypeReferenceNode("Blob");
    },
  });
  return format(
    COMMENT_HEADER +
      "// Regenerate with task api:generate. Browser supplies Origin; binary parts accept Blob/File.\n" +
      astToString(nodes),
    { parser: "typescript", printWidth: 80 },
  );
}
