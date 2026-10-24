# best_loupe

Fast macOS-style hold-to-zoom lens magnifier for Windows (D3D11 + Desktop Duplication)

> **Visibility:** Undecided

## Directives

- All configuration (URLs, API keys, DB credentials, hostnames) **must** come from configuration files, never hardcoded. Prefer stable file-based formats (yaml, toml) for config; use environment variables **only** when there is genuinely no other way (e.g. a secret that must not touch disk, or a platform that injects config solely via env). Use generic placeholders in examples.
- **Read `docs/` before planning** — always consult existing research, findings, and design docs before proposing changes or starting new work. Start from the docs index (`docs/index.md`) to find the relevant document. The docs contain hard-won knowledge about API quirks, country-specific parsing rules, and known limitations.
- **Update `docs/` before committing** — when implementation reveals new findings, edge cases, or changes to architecture/decisions, update the relevant docs first, and keep the docs index (`docs/index.md`) in sync. Docs must stay in sync with the code. If a doc doesn't exist for a new area, create one and add it to the index.
- **Language** — all committed artifacts (code, comments, identifiers, commit messages, docs, `CLAUDE.md`) are written in **English**. The agent's conversation with the user may be in **any language** — match the user; only the artifacts are English-only.

## Project notes

- Rust + `windows` crate; Win32/D3D11/DXGI/DirectComposition. Architecture: `docs/design.md`.
- Build/run on the Windows host via `scripts/win-build.sh` / `scripts/win-run.sh` (see `docs/build.md`); signed uiAccess install via `scripts/deploy.sh local|<ssh-host>` (see `docs/deploy.md`).
- Dependencies are pinned exactly and `Cargo.lock` is committed; check crate publish dates (>= 3 days) on every change.

## Versioning & releases

- SemVer. A release = bump `version` in `Cargo.toml`, add a `CHANGELOG.md` section, commit, annotated tag `vX.Y.Z` on that commit, push commit + tag to Gitea. The agent manages versions itself: minor bump for features, patch for fixes (pre-1.0).
- The version is burned into the binary by `build.rs`: `CARGO_PKG_VERSION` → VERSIONINFO `FileVersion`; `git describe --tags --dirty` (passed via `.build-describe` by `scripts/win-build.sh`) → `ProductVersion`, `crate::BUILD`, log line. `build.rs` warns if the tag and `Cargo.toml` disagree.
- Deploy released builds from a clean tagged tree so the binary reports exactly `vX.Y.Z`.

## Sibling-project initialization

- If you create a **sibling project** (anything at `../<name>` outside this repo), don't leave it as a bare folder. Give it the same baseline setup this project has, before writing substantial code into it:
  - its own `CLAUDE.md` stating the project's purpose, directives, and intended visibility;
  - a `docs/` folder with an index (`docs/index.md`) for design notes and findings;
  - its own initialized git repository with a sensible `.gitignore`.
- First decide and record the new project's visibility (private vs. public) and apply matching precautions — e.g. keep personal data and internal hostnames/IPs out of anything that may become public.

## Visibility-undecided rules (private by default)

- Visibility is **not yet decided** — treat the repo as **private by default** and apply the standard precautions below.
- **Never expose personal data** in code, comments, examples, commit messages, docs, or any committed file: IP addresses, hostnames, domain names of personal infra, API keys/tokens/passwords, asset/user IDs, GPS coordinates of real photos, or paths/filenames that reveal personal info. Store deployment-specific details in Claude memory (outside repo scope), not in project files.
- Lives in private Gitea by default. **Publish to GitHub only on an explicit instruction** from the user — never on your own initiative.
