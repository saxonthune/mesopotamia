/**
 * Luminous pipeline: Rust file/function structure.
 *
 * Stage 2 of 2. Reads `rust-structure.json` (emitted by the
 * tools/luminous-extractor Rust binary) and writes a v3 graph + a domain pack:
 *
 *   rust-structure.json  ──►  rust-structure.graph.json
 *                             rust-structure.pack.json
 *
 * Each file becomes a container node; each function a card nested inside it via
 * a `contains` edge. Free-function calls discovered by the AST walk become
 * `calls` arrows between function cards.
 *
 * Run from the repo root:  tsx .canvases/rust-structure.pipeline.ts
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const IN = join(here, "rust-structure.json");
const GRAPH_OUT = join(here, "rust-structure.graph.json");
const PACK_OUT = join(here, "rust-structure.pack.json");

interface FileEntry {
  path: string;
  function_count: number;
}
interface FunctionOut {
  id: string;
  file: string;
  name: string;
  qualname: string;
  signature: string;
  doc: string;
  public: boolean;
  free: boolean;
  lines: number;
}
interface CallEdge {
  from: string;
  to: string;
}
interface Structure {
  files: FileEntry[];
  functions: FunctionOut[];
  calls: CallEdge[];
}

const fileId = (path: string) => `file.${path}`;

// ── nodes ────────────────────────────────────────────────────────────────
function buildNodes(s: Structure) {
  const fileNodes = s.files.map((f) => ({
    id: fileId(f.path),
    kind: "rust.file",
    props: { path: f.path, functionCount: f.function_count },
    tags: [],
  }));

  const fnNodes = s.functions.map((fn) => ({
    id: fn.id,
    kind: "rust.function",
    props: {
      qualname: fn.qualname,
      signature: fn.signature,
      doc: fn.doc,
      public: fn.public,
      free: fn.free,
      lines: fn.lines,
    },
    tags: fn.public ? ["public"] : ["private"],
  }));

  return [...fileNodes, ...fnNodes];
}

// ── edges ────────────────────────────────────────────────────────────────
function buildEdges(s: Structure) {
  const contains = s.functions.map((fn) => ({
    id: `edge.contains.${fileId(fn.file)}.${fn.id}`,
    kind: "rust.contains",
    from: fileId(fn.file),
    to: fn.id,
    props: {},
    tags: [],
  }));

  const calls = s.calls.map((c) => ({
    id: `edge.calls.${c.from}.${c.to}`,
    kind: "rust.calls",
    from: c.from,
    to: c.to,
    props: {},
    tags: [],
  }));

  return [...contains, ...calls];
}

// ── pack ─────────────────────────────────────────────────────────────────
function buildPack() {
  return {
    id: "rust-structure",
    version: "0.1.0",
    description:
      "Rust source files as containers, functions as cards, free-function calls as arrows.",
    nodeKinds: [
      {
        id: "rust.file",
        label: "File",
        props: {
          type: "object",
          properties: {
            path: { type: "string" },
            functionCount: { type: "integer" },
          },
          required: ["path"],
          additionalProperties: false,
        },
        render: {
          card: {
            type: "card",
            children: [
              { type: "text", value: "{content.path}", style: "heading" },
              { type: "badge", value: "{content.functionCount} fns", tone: "muted" },
            ],
          },
        },
      },
      {
        id: "rust.function",
        label: "Function",
        props: {
          type: "object",
          properties: {
            qualname: { type: "string" },
            signature: { type: "string" },
            doc: { type: "string" },
            public: { type: "boolean" },
            free: { type: "boolean" },
            lines: { type: "integer" },
          },
          required: ["qualname"],
          additionalProperties: false,
        },
        render: {
          peek: {
            type: "card",
            children: [
              { type: "text", value: "{content.qualname}", style: "heading" },
            ],
          },
          card: {
            type: "card",
            children: [
              { type: "text", value: "{content.qualname}", style: "heading" },
              { type: "text", value: "{content.signature}", style: "caption" },
              { type: "badge", value: "{content.lines} ln", tone: "muted" },
            ],
          },
          open: {
            type: "card",
            children: [
              { type: "text", value: "{content.qualname}", style: "heading" },
              { type: "text", value: "{content.signature}", style: "caption" },
              { type: "text", value: "{content.doc}", style: "body" },
              { type: "badge", value: "{content.lines} ln", tone: "muted" },
            ],
          },
        },
      },
    ],
    edgeKinds: [
      {
        id: "rust.contains",
        label: "contains",
        directed: true,
        props: { type: "object", properties: {}, additionalProperties: false },
        acceptsSource: ["rust.file"],
        acceptsTarget: ["rust.function"],
      },
      {
        id: "rust.calls",
        label: "calls",
        directed: true,
        props: { type: "object", properties: {}, additionalProperties: false },
        acceptsSource: ["rust.function"],
        acceptsTarget: ["rust.function"],
      },
    ],
    views: [
      {
        id: "structure",
        name: "File & Function Structure",
        description:
          "Files contain their functions; arrows are free-function calls.",
        zoomToLevel: [
          { minZoom: 0, level: "peek" },
          { minZoom: 0.5, level: "card" },
          { minZoom: 1.4, level: "open" },
        ],
        nodeRoles: { "rust.file": "spatial", "rust.function": "spatial" },
        edgeRoles: { "rust.contains": "contain", "rust.calls": "arrow" },
        layers: {},
        layout: { algorithm: "elk" },
      },
    ],
    layers: [],
    disclosure: [
      {
        kind: "rust.function",
        peek: ["qualname"],
        card: ["qualname", "signature", "lines"],
        open: ["qualname", "signature", "doc", "lines"],
        deep: ["qualname", "signature", "doc", "public", "free", "lines"],
      },
      {
        kind: "rust.file",
        peek: ["path"],
        card: ["path", "functionCount"],
        open: ["path", "functionCount"],
        deep: ["path", "functionCount"],
      },
    ],
  };
}

// ── main ─────────────────────────────────────────────────────────────────
const structure: Structure = JSON.parse(readFileSync(IN, "utf8"));

const nodes = buildNodes(structure);
const edges = buildEdges(structure);
nodes.sort((a, b) => a.id.localeCompare(b.id));
edges.sort((a, b) => a.id.localeCompare(b.id));

const graph = {
  version: 3,
  pack: "rust-structure",
  nodes,
  edges,
  defaultView: "structure",
};

writeFileSync(GRAPH_OUT, JSON.stringify(graph, null, 2));
writeFileSync(PACK_OUT, JSON.stringify(buildPack(), null, 2));

console.error(
  `wrote ${nodes.length} nodes, ${edges.length} edges → rust-structure.graph.json`,
);
