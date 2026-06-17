# mesopotamia

An artistic game-engine visualization of debt-bubble and ecological-flow dynamics, built in Rust. `DESIGN.md` holds the conceptual premise and the buildable models.

## Read the Docs First

This project keeps its specifications in a `.carta/` docs workspace. The docs are one of the project's two sources of truth — code is the concrete reality, docs are the declarative intent. Treat them as authoritative for *what the artifact is for and what it does*, and reconcile against code when the two disagree.

**Start every orientation at `.carta/MANIFEST.md`.** It is the machine-readable index — summaries, tags, and dependency refs for every doc. Read it, identify the docs relevant to the task, then read only those. Do not read the workspace top to bottom. The codex group (`00-codex/`) explains how the system works; `doc00.04` covers AI retrieval patterns and token budgets.

## Apply the Coding Patterns

Before writing or changing code, read `doc02.01` (Coding Patterns). Its rules are standing — one concept per module, decisions extracted into pure tested functions, feature plugins over a thin root, tunables as params — and apply to every code change without being asked. The easy default of one large module that does everything is the wrong one here.

## Docs Unfold; They Are Not Drafted

Documentation grows like a living system — a one-line doc is a finished doc until the work demands more. Capture what is known in the simplest form, deepen only when the next step requires it, and never invent detail to fill a sparse doc. Sparseness is intentional, not a defect.

When the artifact changes, **rewrite the relevant doc in place.** Do not append `## Status` sections, dated updates, or "originally" notes — those turn docs into layered diaries. The full philosophy and the development loop live in `doc00.02`.

## Docs Describe Intent in Present Tense

Docs state what the artifact does, declaratively. Before writing or editing one, avoid temporal prose: future modals ("will", "shall", "going to"), phase/version language ("v1", "MVP", "Phase 1"), deferral language ("TODO", "for now", "coming soon"), and dated or retrospective framing. Planned work goes in `.todo-tasks/`; historically significant decisions go in an ADR. See `doc00.02` for the complete banned-pattern list and examples.

## Search Without Triggering Approval Prompts

Prefer reads and searches the sandbox can auto-approve. Reach for the dedicated tools first — they read and search files without shell indirection that the sandbox must stop to vet.

**Do:**
- Use the Read tool to read files, passing absolute paths.
- Use the Grep and Glob tools for searching content and finding files.
- Pass absolute paths directly to any command instead of changing directory first.
- Keep Bash invocations simple and single-purpose.

**Do not:**
- Combine `cd` with output redirection in one compound command (e.g. `cd dir && cat x > y`). The sandbox requires manual approval to prevent a path-resolution bypass.
- Chain `cd <dir> && <cmd>` when an absolute path makes the `cd` unnecessary — the directory change can force an approval prompt.
- Use Bash `cat`, `head`, `tail`, `grep`, or `find` when a dedicated tool does the same job.

## Structural Changes Use the `rhidoc` CLI

Content changes are normal file edits. Adding, moving, or removing docs uses the `rhidoc` CLI (`rhidoc make`, `rhidoc move`, `rhidoc delete`, `rhidoc regenerate`) so `MANIFEST.md` stays in sync. Run `rhidoc regenerate` after editing frontmatter directly. Run `rhidoc ai-skill` for the full command reference; see `doc00.00` for conventions and `doc00.03` for cross-reference syntax.
