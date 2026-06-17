//! Rust structure extractor for the Luminous pipeline.
//!
//! Walks a source tree with `syn`, emits one JSON document describing every
//! file, the functions inside it (signature, doc, line span, visibility), and
//! the free-function call edges discovered by an AST walk of each body.
//!
//! Usage: `luminous-extractor <src-dir> <out.json>`
//!
//! The two stages are kept apart on purpose: this binary knows only about Rust
//! syntax and emits a neutral structure. The Luminous-specific shaping (kinds,
//! pack, render) lives in the downstream pipeline that consumes this JSON.

use std::path::{Path, PathBuf};

use quote::ToTokens;
use serde::Serialize;
use syn::spanned::Spanned;
use syn::visit::Visit;

/// One function discovered in the source tree.
#[derive(Serialize)]
struct Function {
    /// File-relative path of the containing file, e.g. `src/grid.rs`.
    file: String,
    /// Bare identifier used for call matching, e.g. `new`.
    name: String,
    /// Display name, qualified by impl/trait/module path, e.g. `Grid::new`.
    qualname: String,
    /// One-line reconstructed signature.
    signature: String,
    /// First `///` doc line, if any.
    doc: String,
    /// Whether the function is `pub` (any flavour).
    public: bool,
    /// `true` for free functions (`fn` at module scope) — the only legal
    /// targets of a call edge under the "free-function calls only" rule.
    free: bool,
    /// Total line span of the item (a clumping metric).
    lines: usize,
    /// Bare callee names found in the body via AST walk.
    #[serde(skip)]
    calls: Vec<String>,
}

/// One source file and the functions it holds.
#[derive(Serialize)]
struct FileEntry {
    path: String,
    function_count: usize,
}

/// A discovered free-function call, by stable function id.
#[derive(Serialize)]
struct CallEdge {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct Structure {
    files: Vec<FileEntry>,
    functions: Vec<FunctionOut>,
    calls: Vec<CallEdge>,
}

/// Serialized function view (id resolved, internal `calls` dropped).
#[derive(Serialize)]
struct FunctionOut {
    id: String,
    file: String,
    name: String,
    qualname: String,
    signature: String,
    doc: String,
    public: bool,
    free: bool,
    lines: usize,
}

/// Stable id for a function node: `fn.<file>:<qualname>`.
fn function_id(file: &str, qualname: &str) -> String {
    format!("fn.{file}:{qualname}")
}

/// Collects bare callee identifiers from `foo(...)` call expressions.
/// Method calls (`x.bar()`) are intentionally ignored.
struct CallCollector {
    calls: Vec<String>,
}

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = node.func.as_ref() {
            if let Some(seg) = p.path.segments.last() {
                self.calls.push(seg.ident.to_string());
            }
        }
        // Recurse so nested calls inside arguments are caught too.
        syn::visit::visit_expr_call(self, node);
    }
}

/// First line of the leading doc comment, trimmed.
fn extract_doc(attrs: &[syn::Attribute]) -> String {
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(nv) = &attr.meta {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
            {
                let line = s.value();
                return line.trim().to_string();
            }
        }
    }
    String::new()
}

/// Reconstruct a compact one-line signature from a `syn::Signature`.
fn signature_string(sig: &syn::Signature) -> String {
    let raw = sig.to_token_stream().to_string();
    // `to_string()` pads punctuation; tighten the common cases for readability.
    raw.replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" ,", ",")
        .replace(" <", "<")
        .replace("< ", "<")
        .replace(" >", ">")
}

fn line_span(spanned: &impl Spanned) -> usize {
    let span = spanned.span();
    let start = span.start().line;
    let end = span.end().line;
    end.saturating_sub(start) + 1
}

/// Record a single function into `out`, building its call list.
fn record_fn(
    file: &str,
    qual_prefix: &str,
    free: bool,
    sig: &syn::Signature,
    attrs: &[syn::Attribute],
    vis: &syn::Visibility,
    block: &syn::Block,
    span_node: &impl Spanned,
    out: &mut Vec<Function>,
) {
    let name = sig.ident.to_string();
    let qualname = if qual_prefix.is_empty() {
        name.clone()
    } else {
        format!("{qual_prefix}::{name}")
    };

    let mut collector = CallCollector { calls: Vec::new() };
    collector.visit_block(block);

    out.push(Function {
        file: file.to_string(),
        name,
        qualname,
        signature: signature_string(sig),
        doc: extract_doc(attrs),
        public: matches!(vis, syn::Visibility::Public(_)),
        free,
        lines: line_span(span_node),
        calls: collector.calls,
    });
}

/// Walk a list of items, descending into inline modules and impl blocks.
fn walk_items(file: &str, mod_prefix: &str, items: &[syn::Item], out: &mut Vec<Function>) {
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                record_fn(
                    file, mod_prefix, true, &f.sig, &f.attrs, &f.vis, &f.block, f, out,
                );
            }
            syn::Item::Impl(imp) => {
                let ty = imp.self_ty.to_token_stream().to_string().replace(' ', "");
                let prefix = if mod_prefix.is_empty() {
                    ty
                } else {
                    format!("{mod_prefix}::{ty}")
                };
                for ii in &imp.items {
                    if let syn::ImplItem::Fn(m) = ii {
                        record_fn(
                            file, &prefix, false, &m.sig, &m.attrs, &m.vis, &m.block, m, out,
                        );
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    let prefix = if mod_prefix.is_empty() {
                        m.ident.to_string()
                    } else {
                        format!("{mod_prefix}::{}", m.ident)
                    };
                    walk_items(file, &prefix, inner, out);
                }
            }
            _ => {}
        }
    }
}

/// Parse one file, returning its functions (empty on parse failure).
fn parse_file(root: &Path, path: &Path) -> Vec<Function> {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    // Prefix with the root dir name so paths read like `src/grid.rs`.
    let rel = match root.file_name() {
        Some(name) => format!("{}/{}", name.to_string_lossy(), rel),
        None => rel,
    };

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip {}: {e}", path.display());
            return Vec::new();
        }
    };
    let ast = match syn::parse_file(&content) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("parse {}: {e}", path.display());
            return Vec::new();
        }
    };

    let mut out = Vec::new();
    walk_items(&rel, "", &ast.items, &mut out);
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let src_dir = args.next().unwrap_or_else(|| "src".to_string());
    let out_path = args
        .next()
        .unwrap_or_else(|| ".canvases/rust-structure.json".to_string());

    let root = PathBuf::from(&src_dir);
    let mut funcs: Vec<Function> = Vec::new();

    let mut rs_files: Vec<PathBuf> = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect();
    rs_files.sort();

    for path in &rs_files {
        funcs.extend(parse_file(&root, path));
    }

    // Build the call edges: source = any function body, target = free functions
    // only, matched by bare name. Multiple matches all connect (heuristic — no
    // name resolution). Dedupe (from, to) pairs.
    let free_by_name: std::collections::HashMap<String, Vec<String>> = {
        let mut m: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for f in funcs.iter().filter(|f| f.free) {
            m.entry(f.name.clone())
                .or_default()
                .push(function_id(&f.file, &f.qualname));
        }
        m
    };

    let mut calls: Vec<CallEdge> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for f in &funcs {
        let from = function_id(&f.file, &f.qualname);
        for callee in &f.calls {
            if let Some(targets) = free_by_name.get(callee) {
                for to in targets {
                    if to == &from {
                        continue; // skip self-recursion edges
                    }
                    if seen.insert((from.clone(), to.clone())) {
                        calls.push(CallEdge {
                            from: from.clone(),
                            to: to.clone(),
                        });
                    }
                }
            }
        }
    }

    // File summaries.
    let mut file_counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for f in &funcs {
        *file_counts.entry(f.file.clone()).or_default() += 1;
    }
    let files: Vec<FileEntry> = file_counts
        .into_iter()
        .map(|(path, function_count)| FileEntry {
            path,
            function_count,
        })
        .collect();

    // Resolve function ids and drop internal call lists.
    let mut functions: Vec<FunctionOut> = funcs
        .iter()
        .map(|f| FunctionOut {
            id: function_id(&f.file, &f.qualname),
            file: f.file.clone(),
            name: f.name.clone(),
            qualname: f.qualname.clone(),
            signature: f.signature.clone(),
            doc: f.doc.clone(),
            public: f.public,
            free: f.free,
            lines: f.lines,
        })
        .collect();

    functions.sort_by(|a, b| a.id.cmp(&b.id));
    calls.sort_by(|a, b| (a.from.clone(), a.to.clone()).cmp(&(b.from.clone(), b.to.clone())));

    let structure = Structure {
        files,
        functions,
        calls,
    };

    let json = serde_json::to_string_pretty(&structure).expect("serialize");
    if let Some(parent) = Path::new(&out_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&out_path, json).expect("write output");
    eprintln!(
        "wrote {out_path}: {} files, {} functions, {} calls",
        structure.files.len(),
        structure.functions.len(),
        structure.calls.len()
    );
}
