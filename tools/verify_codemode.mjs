#!/usr/bin/env node
/**
 * Reproducible Pi QuickJS verification for the Headless MCP code-mode contract.
 *
 * This imports Pi's installed codemode tool and executes the same JavaScript
 * shown in docs/headless-mcp.zh-CN.md inside its real QuickJS sandbox. The two
 * fixture promises have MCP CallToolResult shape; no model or live MCP process
 * is needed to verify client-side composition and data reduction.
 */
import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const piEntry = execFileSync("sh", ["-c", 'realpath "$(command -v pi)"'], {
  encoding: "utf8",
}).trim();
const toolModule = resolve(dirname(piEntry), "../extensions/codemode/tool.js");
const { createCodemodeTool } = await import(pathToFileURL(toolModule));

const script = String.raw`
function data(result) {
  const body = result.structuredContent;
  if (result.isError || !body?.ok) {
    throw new Error(body?.error?.message ?? "rasc request failed");
  }
  return body.data;
}

// In normal Pi use these promises are tools.mcp__rasc__classes/strings calls.
// Fixtures retain their exact CallToolResult shape so this verification stays
// deterministic and does not need an APK or model invocation.
const [classes, strings] = await Promise.all([
  Promise.resolve({
    isError: false,
    structuredContent: {
      schema_version: 1,
      ok: true,
      data: { items: [
        { class_id: "0:0:0", dotted_name: "app.internal.Hidden" },
        { class_id: "0:0:1", dotted_name: "app.crypto.Key" },
        { class_id: "0:0:2", dotted_name: "app.crypto.Cipher" },
      ] },
    },
  }),
  Promise.resolve({
    isError: false,
    structuredContent: {
      schema_version: 1,
      ok: true,
      data: { items: [
        { value: "Authorization" },
        { value: "other" },
        { value: "authorization header" },
      ] },
    },
  }),
]);

const interesting = data(classes).items
  .filter(x => x.dotted_name.includes(".crypto."))
  .sort((a, b) => a.dotted_name.localeCompare(b.dotted_name))
  .slice(0, 1)
  .map(({ class_id, dotted_name }) => ({ class_id, dotted_name }));
const authCount = data(strings).items
  .filter(x => x.value.toLowerCase().includes("authorization"))
  .reduce(count => count + 1, 0);
return { interesting, authCount };
`;

const tool = createCodemodeTool([], { models: false });
const result = await tool.execute("rasc-codemode-verification", { code: script });
const text = result.content
  .filter(block => block.type === "text")
  .map(block => block.text)
  .join("\n");
if (result.isError || !text.startsWith("Script completed\n")) {
  process.stderr.write(`${text}\n`);
  process.exit(1);
}

const marker = "Output:\n";
const markerOffset = text.indexOf(marker);
if (markerOffset < 0) {
  throw new Error(`Pi codemode result has no output marker: ${text}`);
}
const output = text.slice(markerOffset + marker.length).trim();
const actual = JSON.parse(output);
const expected = {
  interesting: [{ class_id: "0:0:2", dotted_name: "app.crypto.Cipher" }],
  authCount: 2,
};
if (JSON.stringify(actual) !== JSON.stringify(expected)) {
  throw new Error(`unexpected QuickJS output: ${JSON.stringify(actual)}`);
}
process.stdout.write(`${JSON.stringify(actual)}\n`);
