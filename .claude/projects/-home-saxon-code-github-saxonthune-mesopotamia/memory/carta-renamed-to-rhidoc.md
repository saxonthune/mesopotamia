---
name: carta-renamed-to-rhidoc
description: The carta docs CLI was renamed to rhidoc; config is .rhidoc.json
metadata:
  type: feedback
---

The docs-workspace CLI formerly called `carta` is now `rhidoc` (binary at `~/.local/bin/rhidoc`). Use `rhidoc regenerate`, `rhidoc make`, `rhidoc move`, etc. — `carta` is no longer on PATH.

The repo-root config file was renamed `.carta.json` → `.rhidoc.json`. Its `root` still points at the `.carta/` workspace directory (the directory itself was NOT renamed, and CLAUDE.md / MANIFEST.md still reference `.carta/`).

**Why:** The tool was renamed upstream; CLAUDE.md and the carta-cli skill still say "carta" but the working binary is rhidoc.

**How to apply:** Reach for `rhidoc <subcommand>` for any structural doc change or manifest regen. Same command surface as the old carta CLI (regenerate, make, delete, move, punch, flatten, rename, cat, tree, ls, bundle, orphans).
