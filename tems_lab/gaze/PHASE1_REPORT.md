# Tem Gaze Phase 1 — Prowl V2 Implementation Report

**Date:** 2026-03-28
**Branch:** `vision-upgrade`
**Status:** Implemented, all compilation gates pass

---

## What Was Built

### 1. Grounding Module (`crates/temm1e-tools/src/grounding.rs`)

Shared coordinate transform utilities for vision-based interaction:

| Function | Purpose | Tests |
|----------|---------|-------|
| `scale_for_api(w, h, max_edge, max_pixels)` | Compute API-constrained dimensions for VLM providers | 5 tests |
| `zoom_to_original(x, y, zoom_w, zoom_h, region)` | Transform zoomed coordinates back to original space | 4 tests |
| `dpi_to_logical(x, y, scale_factor)` | Physical pixels → logical points for Retina/HiDPI | 3 tests |
| `api_to_screen(x, y, scale_factor)` | Inverse of scale_for_api for coordinates | 2 tests |
| `clamp_coords(x, y, w, h)` | Clamp to valid screen bounds | 3 tests |
| `validate_zoom_region(x1, y1, x2, y2, w, h)` | Validate and clamp zoom region | 3 tests |

**Total: 6 functions, 20 unit tests, all passing.**

### 2. zoom_region Browser Action (`crates/temm1e-tools/src/browser.rs`)

New action added to the browser tool that captures a specific region of the page at 2x resolution:

- **Parameters:** `x1`, `y1`, `x2`, `y2` (region bounding box in page coordinates)
- **Implementation:** Uses CDP `CaptureScreenshotParams` with `Viewport` clip
- **Scale:** 2.0x for sharper detail in zoomed views
- **Output:** PNG stored as `last_image` for vision pipeline injection
- **Guidance:** Response tells the LLM to use coordinates from the ORIGINAL page, not the zoomed view

**Use case:** LLM identifies a small element on a screenshot but isn't confident about exact coordinates. It calls `zoom_region` on that area, gets a 2x magnified view, and can then `click_at` with more precision.

### 3. SoM Overlay on Tier 3 Observations (`crates/temm1e-tools/src/browser.rs`)

Enhanced the `observe` action's Tier 3 (screenshot) path:

- **Before screenshot:** Injects numbered red circle overlays on all visible interactive and semantic elements
- **Overlay style:** Matches existing OTK session overlay pattern (22px red circles, white text, z-index max)
- **Element detection:** Same walk logic as the accessibility tree JS — `isInteractive` + `isSemantic` roles
- **Viewport filtering:** Only overlays elements with visible bounding boxes (>0 width/height, within viewport)
- **Cleanup:** Removes all `.gaze-som-overlay` elements after screenshot capture
- **Non-fatal:** If overlay injection fails, screenshot still captures normally (graceful degradation)
- **Output enhancement:** Response notes that numbered [N] labels match the tree indices

**Use case:** LLM receives both the numbered tree (Tier 1 data) AND a screenshot with matching [N] labels visible. Instead of guessing pixel coordinates, it can say "click element [7]" and look up [7]'s position from the tree.

### 4. Blueprint Bypass (`crates/temm1e-tools/src/prowl_blueprints.rs`)

New `try_blueprint_bypass(url, intent)` function:

- **Checks:** URL domain against 100+ services in the login registry
- **Returns:** `BlueprintAction::UseLoginBlueprint { service, login_url }` on match
- **Returns:** `None` if no match (fall through to normal grounding)
- **Also added:** `known_service_names()` iterator and made `extract_domain()` public in `login_registry.rs`

**Use case:** Before running the vision pipeline, check if the current URL matches a known service. If so, the login blueprint already knows the selectors — skip vision entirely (0 LLM calls).

### 5. GazeConfig (`crates/temm1e-core/src/types/config.rs`)

New `[gaze]` configuration section:

```toml
[gaze]
enabled = true
high_confidence = 0.85
medium_confidence = 0.40
verify_threshold = 0.70
max_retries = 2
ui_settle_ms = 500
verify_mode = "high_stakes"
monitor = 0
use_accessibility = false

[gaze.browser]
som_overlay = true
zoom_region = true
blueprint_bypass = true
```

All fields have sensible defaults. Existing configs with no `[gaze]` section get defaults automatically via `#[serde(default)]`.

---

## Compilation Gates

| Gate | Result |
|------|--------|
| `cargo check --workspace` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS (0 warnings) |
| `cargo fmt --all -- --check` | PASS |
| `cargo test -p temm1e-tools` | PASS (130 tests, 0 failures) |
| `cargo test -p temm1e-core` | PASS (144 tests, 0 failures, 1 pre-existing skip) |

---

## New Test Inventory

| Module | Test | Status |
|--------|------|--------|
| `grounding` | `scale_for_api_no_scaling_needed` | PASS |
| `grounding` | `scale_for_api_edge_limited` | PASS |
| `grounding` | `scale_for_api_pixel_limited` | PASS |
| `grounding` | `scale_for_api_zero_dimensions` | PASS |
| `grounding` | `scale_for_api_exact_boundary` | PASS |
| `grounding` | `zoom_to_original_full_region_identity` | PASS |
| `grounding` | `zoom_to_original_top_left_quadrant` | PASS |
| `grounding` | `zoom_to_original_offset_region` | PASS |
| `grounding` | `zoom_to_original_zero_dimensions` | PASS |
| `grounding` | `dpi_retina` | PASS |
| `grounding` | `dpi_standard` | PASS |
| `grounding` | `dpi_zero_scale` | PASS |
| `grounding` | `api_to_screen_with_scaling` | PASS |
| `grounding` | `api_to_screen_no_scaling` | PASS |
| `grounding` | `clamp_within_bounds` | PASS |
| `grounding` | `clamp_negative` | PASS |
| `grounding` | `clamp_overflow` | PASS |
| `grounding` | `validate_zoom_valid` | PASS |
| `grounding` | `validate_zoom_x2_less_than_x1` | PASS |
| `grounding` | `validate_zoom_clamped_to_bounds` | PASS |
| `prowl_blueprints` | `blueprint_bypass_known_service` | PASS |
| `prowl_blueprints` | `blueprint_bypass_unknown_site` | PASS |
| `prowl_blueprints` | `blueprint_bypass_invalid_url` | PASS |

**23 new tests, all passing.**

---

## Files Changed

| File | Change Type | Lines |
|------|-------------|-------|
| `crates/temm1e-tools/src/grounding.rs` | NEW | ~260 |
| `crates/temm1e-tools/src/browser.rs` | MODIFIED | ~140 added |
| `crates/temm1e-tools/src/lib.rs` | MODIFIED | +1 line |
| `crates/temm1e-tools/src/prowl_blueprints.rs` | MODIFIED | ~80 added |
| `crates/temm1e-tools/src/prowl_blueprints/login_registry.rs` | MODIFIED | ~15 added |
| `crates/temm1e-core/src/types/config.rs` | MODIFIED | ~100 added |

**Total: ~596 lines of production code + tests. Zero new dependencies.**

---

## What's NOT Yet Built (Phase 2 — Tem Gaze Desktop)

These are designed in DESIGN.md but not implemented in this phase:

- `temm1e-gaze` crate (ScreenController trait, DesktopScreenController)
- `enigo` + `xcap` integration for OS-level input/capture
- Desktop SoM overlay via image compositing
- Desktop tool registration in main.rs
- `--features desktop-control` feature flag

Phase 2 depends on Phase 1 being proven in production. The grounding module, config, and techniques are shared.

---

## Live Validation Plan

Browser features require a real Chrome instance. Validation should be performed by:

1. **Building release binary:** `cargo build --release --bin temm1e`
2. **Launching with browser enabled:** Standard Temm1e launch
3. **Testing zoom_region:**
   - Navigate to any page
   - Take screenshot
   - Call `zoom_region` with a sub-region
   - Verify zoomed image appears in next LLM context
4. **Testing SoM overlay:**
   - Navigate to a form-heavy page (e.g., login form)
   - Call `observe` with `retry=true` (forces Tier 3)
   - Verify screenshot contains numbered red circles
   - Verify tree indices match overlay numbers
5. **Testing blueprint bypass:**
   - Navigate to facebook.com
   - Call `try_blueprint_bypass` internally
   - Verify `UseLoginBlueprint { service: "facebook" }` returned
