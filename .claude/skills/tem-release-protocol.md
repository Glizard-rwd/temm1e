# Skill: Tem Release Protocol

## When to use

Use this skill for EVERY release to main. No exceptions. This is the complete, battle-tested protocol derived from the v3.4.0 Tem Gaze release. It covers everything from compilation gates to README updates to merge and push.

**Trigger phrases:** "release", "merge to main", "push to main", "ship it", "version bump", "prepare release"

## The Protocol

Execute every step in order. Do not skip steps. Do not reorder. Record all metrics as you go.

---

### Phase 1: Compilation Gates

ALL four must pass. If any fails, fix before proceeding.

```bash
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace --all-features
```

If `cargo fmt` fails, run `cargo fmt --all` then re-check.

Record the test count:
```bash
cargo test --workspace --all-features 2>&1 | grep "^test result:" | awk '{sum += $4} END {print "Tests: " sum}'
```

Record the crate count:
```bash
ls crates/ | wc -l
```

Note any pre-existing test failures (failures that also exist on the current main branch). These are acceptable if documented. New failures are NOT acceptable.

---

### Phase 2: Version Bump

**Single source of truth:** `Cargo.toml` line ~27

```toml
[workspace.package]
version = "X.Y.Z"
```

**Versioning rules:**
- **Major (X):** Breaking API changes, architecture overhaul
- **Minor (Y):** New features, new crates, new capabilities
- **Patch (Z):** Bug fixes, documentation, minor improvements

After bumping, run `cargo check --workspace` to verify it propagates to all crates.

---

### Phase 3: README.md Updates

Search and update ALL of these. Use `grep` to find them:

**Version references:**
```bash
grep -n "version.*[0-9]\+\.[0-9]\+\.[0-9]\+" README.md
```
- Version badge: `version-X.Y.Z-blue`
- Any inline version references

**Test count references:**
```bash
grep -n "tests" README.md
```
- Hero line: `<code>N tests</code>`
- Stats table: `<strong>N</strong><br><sub>Tests</sub>`
- Dev section: `cargo test --workspace  # N tests`

**Architecture tree:**
```bash
grep -n "temm1e-" README.md | head -30
```
- Verify ALL crates in `crates/` are listed in the architecture tree
- If a new crate was added, add it in alphabetical position with a one-line description
- Update the temm1e-tools description if new tools were added

**Feature flags:**
- If a new feature flag was added, add build instructions in the appropriate section
- Example format:
  ```
  **Feature Name** (brief description):
  ```bash
  cargo build --release --features feature-name
  ```

**Release timeline:**
- Add new entry at the TOP of the release timeline (inside the `<details>` block)
- Format: `YYYY-MM-DD  vX.Y.Z  ●━━━ One-line summary. Details. Test count`
- Include: what was added, key metrics, test count

---

### Phase 4: CLAUDE.md Updates

```bash
grep -n "crates" CLAUDE.md
```

- Line ~7: Crate count ("The codebase is a Cargo workspace with N crates plus a root binary")
- Workspace structure section: Add new crate entries if applicable
- Any stale references to test counts or feature lists

---

### Phase 5: Setup Documentation

**SETUP_FOR_PROS.md:**
```bash
grep -n "tests" SETUP_FOR_PROS.md
```
- Update test count in the compilation gate comment
- Add new feature flags to the optional features section

**docs/dev/getting-started.md:**
- Update the feature flags table if new features were added
- Mark default features with "— **default**"

**SETUP_FOR_NEWBIE.md:**
- Check for any stale version references

---

### Phase 6: Docker Verification

**Dockerfile:**
- If a new default feature was added, add it to the `FEATURES` ARG (line ~12)
- If a new feature is opt-in only (like `desktop-control`), do NOT add to defaults — document in README instead
- Verify the Rust version in `FROM rust:X.Y-bookworm` is >= the MSRV

**docker-compose.yml:**
- No changes needed unless environment variables changed

---

### Phase 7: CI Verification

**.github/workflows/ci.yml:**
- CI runs `cargo clippy --workspace --all-targets --all-features` — this includes ALL feature-gated crates
- If a new feature flag was added, it is automatically covered by `--all-features`
- **If new crate dependencies require system libraries (C libs, pkg-config):** add `apt-get install` to the `Install system dependencies` step in the `check` job
- Current system deps installed in CI: `libwayland-dev libxcb1-dev libxcb-randr0-dev libxcb-shm0-dev libxkbcommon-dev` (for xcap/desktop-control)
- **Test this locally first:** `cargo clippy --workspace --all-targets --all-features` must pass on your machine before pushing. If it passes locally but fails in CI, the difference is missing system libraries on the Ubuntu runner.

**Common CI failures from new features:**
| Symptom | Cause | Fix |
|---------|-------|-----|
| `pkg-config ... was not found` | New crate needs a C library | Add `apt-get install libXXX-dev` to CI |
| `wayland-sys build failed` | xcap needs Wayland libs on Linux | Already fixed: `libwayland-dev` installed |
| `failed to link` on musl target | C library not available for musl | Either vendor the lib or exclude the feature from musl builds |
| Clippy passes locally (macOS) but fails in CI (Linux) | Platform-specific deps | Add Linux-specific `apt-get install` |

**.github/workflows/release.yml:**
- The release build does NOT use `--all-features` — it builds the default feature set
- `desktop-control` is opt-in and NOT built in release CI (correct — users enable it themselves)
- If a new **default** feature requires system libs, add them to the release workflow too

---

### Phase 8: Final Verification

Re-run ALL compilation gates after all edits:

```bash
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace --all-features 2>&1 | grep "^test result:" | awk '{sum += $4} END {print "Tests: " sum}'
```

Confirm the test count matches what you wrote in README. If it doesn't, go back and fix.

---

### Phase 9: Commit

Stage only release-relevant files. Do NOT stage unrelated files.

```bash
git add Cargo.toml Cargo.lock README.md CLAUDE.md SETUP_FOR_PROS.md docs/dev/getting-started.md [any other changed files]
```

Commit with this format:
```bash
git commit -m "release: vX.Y.Z — one-line feature summary

Detailed bullet points of what changed.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Phase 10: Merge and Push

**If on a feature branch:**
```bash
git checkout main
git merge <branch> --no-ff -m "Merge branch '<branch>' — vX.Y.Z summary"
git push origin main
```

**If on main directly (hotfix):**
```bash
git push origin main
```

**After push:** Verify the CI passes on GitHub. If release CI exists, wait for it to complete and verify artifacts (per feedback_release_ci_wait memory).

---

## Checklist (Copy-Paste for Each Release)

```
[ ] cargo check --workspace — PASS
[ ] cargo clippy --all-features -- -D warnings — PASS
[ ] cargo fmt --check — PASS
[ ] cargo test --all-features — PASS (N tests)
[ ] Cargo.toml version bumped to X.Y.Z
[ ] README version badge updated
[ ] README test count updated (3 places)
[ ] README architecture tree updated (new crates)
[ ] README feature build instructions (new features)
[ ] README release timeline entry added
[ ] CLAUDE.md crate count updated
[ ] CLAUDE.md workspace structure updated
[ ] SETUP_FOR_PROS.md test count updated
[ ] SETUP_FOR_PROS.md optional features updated
[ ] docs/dev/getting-started.md feature table updated
[ ] Dockerfile verified (default features)
[ ] CI workflows verified (new features + system deps)
[ ] CI system deps updated if new C libraries needed
[ ] Final compilation gates re-passed
[ ] Test count matches README
[ ] Committed and pushed
[ ] CI passes on GitHub
```

---

## Common Mistakes (Learned from Real Releases)

| Mistake | Consequence | Prevention |
|---------|-------------|------------|
| Bump README but not Cargo.toml | `temm1e -V` shows old version | Always bump Cargo.toml FIRST |
| Forget test count in 3 README locations | Stale metrics visible to users | `grep -n "tests" README.md` |
| Forget architecture tree entry | New crate invisible in docs | `ls crates/` and cross-reference |
| Forget CLAUDE.md crate count | Claude starts with wrong context | Check every release |
| Push without re-running tests | Edits broke something | Phase 8 is mandatory |
| Feature-gated crate not in workspace members | `cargo check` doesn't catch errors | `grep members Cargo.toml` |
| Feature flag chain incomplete | `--features X` doesn't enable dependency | Root feature must enable child crate feature |
| MCP Playwright left in config | 5.7x cost multiplier, tool confusion | Removed in v3.4.0, don't re-add |
| Stale test count from earlier version | Users see wrong number, lose trust | Always re-count with `cargo test --all-features` |
| New crate needs C library, CI not updated | CI fails with `pkg-config ... not found` | Add `apt-get install` to CI check job |
| Clippy passes on macOS, fails on Linux CI | Platform-specific system deps missing | Test `--all-features` locally AND check CI deps |
| Feature-gated crate pulls in system lib | `--all-features` in CI triggers build of all crates | Add system dep to CI, or exclude feature from CI |

---

## Version History of This Protocol

- **v2 (v3.4.0 hotfix, 2026-03-28):** Added CI/CD section with system dependency management, common CI failure table, release.yml vs ci.yml feature scope distinction. Triggered by `wayland-sys` build failure in CI after desktop-control feature was added.
- **v1 (v3.4.0, 2026-03-28):** Created from Tem Gaze release. Covers: compilation gates, version bump, README (badges, test counts, architecture tree, release timeline, feature instructions), CLAUDE.md, SETUP_FOR_PROS.md, getting-started.md, Dockerfile, CI, merge workflow. Derived from actual mistakes made during the release process.
