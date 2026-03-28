# Tem Gaze: Implementation Plan

**Date:** 2026-03-28
**Design:** [DESIGN.md](DESIGN.md)
**Research:** [RESEARCH_PAPER.md](RESEARCH_PAPER.md)

---

## Overview

Implementation is split into two phases: Prowl V2 (browser vision upgrade, no new crates) and Tem Gaze (desktop control, new `temm1e-gaze` crate). Prowl V2 ships first because it enhances existing functionality with zero new dependencies.

---

## Phase 1: Prowl V2 — Browser Vision Upgrade

### P1.1: Shared Grounding Module

**New file:** `crates/temm1e-tools/src/grounding.rs`

This module contains the model-agnostic grounding logic shared by browser and desktop tools.

```rust
// crates/temm1e-tools/src/grounding.rs

/// Configuration for the grounding pipeline
pub struct GroundingConfig {
    pub high_confidence: f64,       // Default 0.85
    pub medium_confidence: f64,     // Default 0.40
    pub verify_threshold: f64,      // Default 0.70
    pub max_retries: u32,           // Default 2
    pub ui_settle_ms: u64,          // Default 500
    pub verify_mode: VerifyMode,    // Default HighStakes
}

pub enum VerifyMode {
    Off,
    HighStakes,  // Verify destructive actions only
    Always,
}

/// Result of a coarse grounding pass
pub struct CoarseResult {
    pub description: String,
    pub confidence: f64,
    pub region: [u32; 4],       // [x1, y1, x2, y2]
    pub coordinates: [u32; 2],  // [x, y] best estimate
}

/// Result of a zoom-refine pass
pub struct RefineResult {
    pub description: String,
    pub confidence: f64,
    pub coordinates: [u32; 2],  // [x, y] in original space
}

/// Result of SoM selection
pub struct SomResult {
    pub element_index: u32,
    pub confidence: f64,
    pub coordinates: [u32; 2],  // Centroid of selected element
}

/// Result of action verification
pub struct VerifyResult {
    pub success: bool,
    pub confidence: f64,
    pub actual_change: String,
}

// --- Prompt rendering ---

pub fn render_coarse_prompt(intent: &str, width: u32, height: u32) -> String;
pub fn render_refine_prompt(intent: &str, width: u32, height: u32) -> String;
pub fn render_som_prompt(intent: &str) -> String;
pub fn render_verify_prompt(intent: &str, x: u32, y: u32) -> String;

// --- Response parsing ---

pub fn parse_coarse_response(response: &str) -> Result<CoarseResult, Temm1eError>;
pub fn parse_refine_response(response: &str) -> Result<RefineResult, Temm1eError>;
pub fn parse_som_response(response: &str) -> Result<SomResult, Temm1eError>;
pub fn parse_verify_response(response: &str) -> Result<VerifyResult, Temm1eError>;

// --- Coordinate transforms ---

/// Transform coordinates from zoomed space back to original space
pub fn zoom_to_original(
    x_zoomed: u32, y_zoomed: u32,
    zoom_width: u32, zoom_height: u32,
    region: [u32; 4],  // [x1, y1, x2, y2] in original space
) -> (u32, u32);

/// Scale screenshot for API constraints, return (scaled_bytes, scale_factor)
pub fn scale_for_api(
    png_bytes: &[u8],
    max_longest_edge: u32,  // e.g., 1568
    max_total_pixels: u32,  // e.g., 1_150_000
) -> Result<(Vec<u8>, f64), Temm1eError>;

/// Crop a region from a PNG and resize to fill API constraints
pub fn crop_and_resize(
    png_bytes: &[u8],
    x1: u32, y1: u32, x2: u32, y2: u32,
    max_longest_edge: u32,
    max_total_pixels: u32,
) -> Result<(Vec<u8>, f64), Temm1eError>;
```

**Tests (in same file):**
- `test_zoom_to_original_identity` — region covers full image, transform is identity
- `test_zoom_to_original_quadrant` — region is top-left quarter, verify correct scaling
- `test_zoom_to_original_small_region` — tiny region, verify precision
- `test_scale_for_api_no_scaling` — image within API limits, no change
- `test_scale_for_api_downscale` — image exceeds limits, verify correct dimensions
- `test_crop_and_resize` — crop to region, verify output dimensions
- `test_parse_coarse_response_valid` — well-formed JSON parses correctly
- `test_parse_coarse_response_malformed` — graceful error on bad JSON
- `test_confidence_threshold_routing` — verify HIGH/MED/LOW routes correctly

### P1.2: Generalize SoM Overlay for Browser

**Modified file:** `crates/temm1e-tools/src/browser.rs`

Extend the `observe` action (Tier 3) to include SoM overlays.

**Changes:**
1. When `select_tier()` returns Tier 3, also collect element bounding boxes
2. Inject SoM overlay labels via CDP JavaScript execution
3. Capture screenshot with overlays visible
4. Remove overlays via CDP JavaScript execution
5. Return annotated screenshot + accessibility tree with matching [N] indices

```rust
// In the observe action handler, after Tier 3 is selected:

// 1. Get interactive elements with bounding boxes
let elements = self.get_interactive_elements_with_bounds(&page).await?;

// 2. Inject SoM overlays
let overlay_js = build_som_overlay_js(&elements);
page.evaluate(overlay_js).await?;

// 3. Capture screenshot (now includes overlays)
let screenshot = page.screenshot(/* ... */).await?;

// 4. Clean up overlays
page.evaluate("document.querySelectorAll('[data-gaze-overlay]').forEach(e => e.remove())").await?;

// 5. Build response with indexed tree + annotated screenshot
```

**New helper function:**
```rust
async fn get_interactive_elements_with_bounds(
    &self, page: &Page
) -> Result<Vec<(usize, String, String, Rect)>, Temm1eError> {
    // Execute JS that walks the accessibility tree and calls
    // getBoundingClientRect() on each interactive element
    // Returns: [(index, role, name, {x, y, width, height}), ...]
}

fn build_som_overlay_js(elements: &[(usize, String, String, Rect)]) -> String {
    // Generate JS that injects numbered red circles at each element's position
    // Same visual style as existing OVERLAY_INJECT_JS in browser_session.rs
}
```

**Tests:**
- `test_som_overlay_js_generation` — correct JS for N elements
- `test_som_overlay_cleanup` — overlays removed after screenshot

### P1.3: Add `zoom_region` Browser Action

**Modified file:** `crates/temm1e-tools/src/browser.rs`

Add `zoom_region` to the browser action enum and handler.

```rust
// In the action match block:
"zoom_region" => {
    let x1 = get_param_u32(params, "x1")?;
    let y1 = get_param_u32(params, "y1")?;
    let x2 = get_param_u32(params, "x2")?;
    let y2 = get_param_u32(params, "y2")?;

    // Validate region bounds
    ensure!(x2 > x1 && y2 > y1, "Invalid zoom region");

    let screenshot = page.screenshot(/* PNG */).await?;
    let (cropped, _scale) = grounding::crop_and_resize(
        &screenshot, x1, y1, x2, y2,
        1568,       // Anthropic max edge
        1_150_000,  // Anthropic max pixels
    )?;

    self.last_image = Some(ToolOutputImage {
        media_type: "image/png".into(),
        data: base64::encode(&cropped),
    });

    Ok(ToolOutput::text(format!(
        "Zoomed into region ({x1},{y1})-({x2},{y2}) at full resolution. \
         Analyze this view for precise element identification."
    )))
}
```

Update tool declaration to include `zoom_region` action and `x1`, `y1`, `x2`, `y2` parameters.

**Tests:**
- `test_zoom_region_valid` — mock page, verify crop dimensions
- `test_zoom_region_invalid` — x2 < x1, verify error
- `test_zoom_region_out_of_bounds` — region exceeds page, verify clamping

### P1.4: Blueprint Bypass Enhancement

**Modified file:** `crates/temm1e-tools/src/prowl_blueprints.rs`

Add a `try_blueprint_bypass()` function called before vision grounding:

```rust
/// Check if a known blueprint can handle this action without vision
pub fn try_blueprint_bypass(
    url: &str,
    intent: &str,
) -> Option<BlueprintAction> {
    // 1. Match URL against login_registry (100+ services)
    // 2. If match, return known selectors for the action
    // 3. Return None if no blueprint matches
}

pub enum BlueprintAction {
    ClickSelector(String),      // CSS selector to click
    TypeInSelector(String, String), // (selector, text)
    Sequence(Vec<BlueprintAction>), // Multi-step flow
}
```

**Tests:**
- `test_blueprint_bypass_facebook_login` — known URL, returns selector
- `test_blueprint_bypass_unknown_site` — unknown URL, returns None
- `test_blueprint_bypass_partial_match` — similar URL, no false positive

### P1.5: Compilation Gate

After all P1 changes:

```bash
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

All must pass. Run `cargo test -p temm1e-tools` specifically for grounding module tests.

---

## Phase 2: Tem Gaze — Desktop Control

### P2.1: ScreenController Trait

**New file:** `crates/temm1e-core/src/traits/screen.rs`

Define the trait as specified in DESIGN.md Section 2.2. Register in `crates/temm1e-core/src/traits/mod.rs`.

**Types to add to `crates/temm1e-core/src/types/`:**
- `Screenshot` struct
- `A11yElement` struct
- `Rect` struct
- `ScrollDirection` enum

**Tests:**
- Type serialization/deserialization
- Rect boundary validation

### P2.2: temm1e-gaze Crate

**New crate:** `crates/temm1e-gaze/`

Create the full crate structure as specified in DESIGN.md Section 6.1.

**Files:**

```
crates/temm1e-gaze/
    Cargo.toml              -- Dependencies as in DESIGN.md Section 6.2
    src/
        lib.rs              -- pub mod desktop_controller, overlay, coordinate, platform;
        desktop_controller.rs -- DesktopScreenController implementing ScreenController
        overlay.rs          -- SoM overlay compositing on screenshot PNGs
        coordinate.rs       -- DPI scaling, API resolution transforms
        platform/
            mod.rs          -- #[cfg] dispatch to platform modules
            macos.rs        -- Retina detection, key name mapping
            linux.rs        -- X11/Wayland detection, key name mapping
```

**Key implementation: `desktop_controller.rs`**

As specified in DESIGN.md Section 6.3. Core methods:

- `new(monitor_index: usize) -> Result<Self, Temm1eError>`
- `capture()` — `xcap::Monitor::capture_image()` → PNG bytes
- `click(x, y)` — `enigo::move_mouse()` + `enigo::button(Left, Click)`
- `double_click(x, y)` — `enigo::move_mouse()` + `enigo::button(Left, Click)` x2
- `right_click(x, y)` — `enigo::move_mouse()` + `enigo::button(Right, Click)`
- `type_text(text)` — `enigo::text()`
- `key_combo(keys)` — `enigo::key()` with modifier parsing
- `scroll(x, y, dir, amount)` — `enigo::move_mouse()` + `enigo::scroll()`
- `drag(x1, y1, x2, y2)` — `enigo::move_mouse()` + `enigo::button(Left, Press)` + `enigo::move_mouse()` + `enigo::button(Left, Release)`
- `dimensions()` — `xcap::Monitor` logical dimensions
- `scale_factor()` — `xcap::Monitor` scale factor
- `accessibility_tree()` — feature-gated, returns `None` by default

**Key implementation: `overlay.rs`**

```rust
use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_text_mut};

/// Composite numbered SoM labels onto a screenshot PNG
pub fn overlay_som_labels(
    screenshot_png: &[u8],
    candidates: &[(u32, u32, u32)],  // (index, center_x, center_y)
) -> Result<Vec<u8>, Temm1eError> {
    let mut img = image::load_from_memory(screenshot_png)?;

    for &(index, cx, cy) in candidates {
        // Draw red filled circle (radius 14)
        draw_filled_circle_mut(
            &mut img, (cx as i32, cy as i32), 14,
            Rgba([229, 62, 62, 255])  // #e53e3e
        );
        // Draw white number text centered in circle
        // Use embedded font (no external font file dependency)
        draw_index_label(&mut img, index, cx, cy);
    }

    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;
    Ok(out)
}
```

**Key implementation: `coordinate.rs`**

All coordinate transform functions as specified in DESIGN.md Section 3.2. Pure functions, extensively tested.

**Tests per file:**
- `desktop_controller.rs`: `test_new_primary_monitor`, `test_dimensions_logical`, `test_scale_factor_retina`, `test_scale_factor_standard`
- `overlay.rs`: `test_overlay_single_label`, `test_overlay_multiple_labels`, `test_overlay_preserves_dimensions`, `test_overlay_empty_candidates`
- `coordinate.rs`: `test_dpi_scaling_retina`, `test_dpi_scaling_standard`, `test_api_scaling_within_limits`, `test_api_scaling_exceeds_limits`, `test_zoom_transform_full_region`, `test_zoom_transform_partial_region`
- `platform/macos.rs`: `test_key_mapping_modifiers`, `test_key_mapping_special_keys`
- `platform/linux.rs`: `test_key_mapping_modifiers`, `test_wayland_detection`

### P2.3: Desktop Tool

**New file:** `crates/temm1e-tools/src/desktop_tool.rs`

Implements the `Tool` trait for desktop control, using `DesktopScreenController` and the shared grounding pipeline.

```rust
pub struct DesktopTool {
    controller: Arc<DesktopScreenController>,
    grounding_config: GroundingConfig,
    last_image: Option<ToolOutputImage>,
}

#[async_trait]
impl Tool for DesktopTool {
    fn declarations(&self) -> Vec<ToolDeclaration> {
        // Desktop tool schema as in DESIGN.md Section 2.4
    }

    async fn execute(&mut self, params: &Value) -> Result<ToolOutput, Temm1eError> {
        match params["action"].as_str() {
            Some("screenshot") => self.handle_screenshot().await,
            Some("click") => self.handle_click(params).await,
            Some("double_click") => self.handle_double_click(params).await,
            Some("right_click") => self.handle_right_click(params).await,
            Some("type") => self.handle_type(params).await,
            Some("key") => self.handle_key(params).await,
            Some("scroll") => self.handle_scroll(params).await,
            Some("drag") => self.handle_drag(params).await,
            Some("observe") => self.handle_observe(params).await,
            Some("zoom_region") => self.handle_zoom_region(params).await,
            _ => Err(Temm1eError::Tool("Unknown desktop action".into())),
        }
    }

    fn take_last_image(&mut self) -> Option<ToolOutputImage> {
        self.last_image.take()
    }
}
```

**Action handlers follow the grounding pipeline:**
- `handle_screenshot()` — Capture and store as `last_image`
- `handle_click()` — Takes (x, y), applies DPI transform, executes click
- `handle_observe()` — Capture screenshot, optionally query a11y, return combined observation
- `handle_zoom_region()` — Crop region, resize, store as `last_image`

### P2.4: Wire Desktop Tool into Agent

**Modified file:** `crates/temm1e-tools/src/lib.rs`

Add `desktop_tool` module (feature-gated):

```rust
#[cfg(feature = "desktop-control")]
pub mod desktop_tool;
```

**Modified file:** `src/main.rs`

Register desktop tool alongside existing tools when `[gaze] enabled = true`:

```rust
if config.gaze.enabled {
    #[cfg(feature = "desktop-control")]
    {
        let desktop = DesktopTool::new(config.gaze.clone())?;
        tools.push(Box::new(desktop));
    }
    #[cfg(not(feature = "desktop-control"))]
    {
        tracing::warn!("Desktop control enabled in config but binary was not compiled with --features desktop-control");
    }
}
```

### P2.5: Config Extension

**Modified file:** `crates/temm1e-core/src/types/config.rs`

Add `GazeConfig` struct:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GazeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_high_confidence")]
    pub high_confidence: f64,
    #[serde(default = "default_medium_confidence")]
    pub medium_confidence: f64,
    #[serde(default = "default_verify_threshold")]
    pub verify_threshold: f64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_ui_settle_ms")]
    pub ui_settle_ms: u64,
    #[serde(default = "default_verify_mode")]
    pub verify_mode: String,
    #[serde(default)]
    pub monitor: usize,
    #[serde(default)]
    pub use_accessibility: bool,
    #[serde(default)]
    pub browser: GazeBrowserConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GazeBrowserConfig {
    #[serde(default = "default_true")]
    pub som_overlay: bool,
    #[serde(default = "default_true")]
    pub zoom_region: bool,
    #[serde(default = "default_true")]
    pub blueprint_bypass: bool,
}

fn default_high_confidence() -> f64 { 0.85 }
fn default_medium_confidence() -> f64 { 0.40 }
fn default_verify_threshold() -> f64 { 0.70 }
fn default_max_retries() -> u32 { 2 }
fn default_ui_settle_ms() -> u64 { 500 }
fn default_verify_mode() -> String { "high_stakes".into() }
fn default_true() -> bool { true }
```

Add `gaze: GazeConfig` field to the root `Config` struct with `#[serde(default)]`.

**Tests:**
- `test_gaze_config_defaults` — empty `[gaze]` section uses all defaults
- `test_gaze_config_override` — each field can be overridden
- `test_gaze_config_missing` — no `[gaze]` section produces disabled config

### P2.6: Workspace Integration

**Modified file:** `Cargo.toml` (root workspace)

Add `temm1e-gaze` to workspace members:

```toml
[workspace]
members = [
    # ... existing crates ...
    "crates/temm1e-gaze",
]
```

Add feature flag:

```toml
[features]
desktop-control = ["temm1e-gaze"]
```

Add optional dependency:

```toml
[dependencies]
temm1e-gaze = { path = "crates/temm1e-gaze", optional = true }
```

### P2.7: Compilation Gate

```bash
# Without desktop-control (must be byte-identical to pre-Gaze)
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# With desktop-control
cargo check --workspace --features desktop-control
cargo clippy --workspace --all-targets --features desktop-control -- -D warnings
cargo test --workspace --features desktop-control

# Format
cargo fmt --all -- --check
```

All must pass on both with and without the feature flag.

---

## Phase 3: Validation and Benchmarks

### P3.1: Browser Grounding Benchmark

Capture 20 screenshots of web pages at varying complexity:
- 5 simple pages (< 10 interactive elements)
- 5 medium pages (10-30 elements)
- 5 dense pages (30-100 elements)
- 5 complex pages (nested menus, small icons)

For each screenshot, manually annotate 3 target elements with ground-truth bounding boxes.

Test each grounding strategy:
- Raw `click_at(x, y)` (current baseline)
- SoM overlay + element selection
- Zoom-refine
- Zoom-refine + SoM fallback

Metrics: accuracy (click within target bounds), token cost, latency.

### P3.2: Desktop Grounding Benchmark

Capture 20 desktop screenshots:
- 5 simple (Finder, text editor)
- 5 medium (Settings, file manager with files)
- 5 dense (IDE, terminal with output)
- 5 complex (multi-window, small toolbar icons)

Same annotation and testing protocol as browser benchmark.

### P3.3: End-to-End Task Validation

Script 5 desktop tasks:
1. Open a text editor, type "Hello World", save the file
2. Open a terminal, run `ls -la`, capture the output
3. Open System Settings, navigate to a specific setting
4. Open a file manager, rename a file
5. Open two windows, drag a file from one to the other

Validate: task completes successfully, all clicks hit targets, verify-retry catches misclicks.

---

## Dependency Summary

### New Crate Dependencies (feature-gated behind `desktop-control`)

| Crate | Version | Purpose | Size |
|-------|---------|---------|------|
| `enigo` | 0.3.x | Cross-platform input simulation | ~50 KB |
| `xcap` | 0.8.x | Cross-platform screen capture | ~30 KB |
| `image` | 0.25.x | PNG manipulation for SoM overlay | Already transitive dep |

### Existing Dependencies Used

| Crate | Purpose |
|-------|---------|
| `async-trait` | Trait async methods |
| `serde` / `serde_json` | Config and response parsing |
| `tracing` | Structured logging |
| `tokio` | Async runtime, timers |
| `base64` | Screenshot encoding |

### Zero New Dependencies for Browser-Only Users

When `desktop-control` feature is OFF (default):
- `enigo` not compiled
- `xcap` not compiled
- Binary size unchanged
- Behavior unchanged
- Prowl V2 enhancements use only existing dependencies (`image` is already transitive via `chromiumoxide`)

---

## File Change Summary

### New Files

| File | Crate | Purpose |
|------|-------|---------|
| `crates/temm1e-gaze/Cargo.toml` | temm1e-gaze | Crate manifest |
| `crates/temm1e-gaze/src/lib.rs` | temm1e-gaze | Module root |
| `crates/temm1e-gaze/src/desktop_controller.rs` | temm1e-gaze | ScreenController impl |
| `crates/temm1e-gaze/src/overlay.rs` | temm1e-gaze | SoM image compositing |
| `crates/temm1e-gaze/src/coordinate.rs` | temm1e-gaze | Transform math |
| `crates/temm1e-gaze/src/platform/mod.rs` | temm1e-gaze | Platform dispatch |
| `crates/temm1e-gaze/src/platform/macos.rs` | temm1e-gaze | macOS specifics |
| `crates/temm1e-gaze/src/platform/linux.rs` | temm1e-gaze | Linux specifics |
| `crates/temm1e-core/src/traits/screen.rs` | temm1e-core | ScreenController trait |
| `crates/temm1e-tools/src/grounding.rs` | temm1e-tools | Shared grounding pipeline |
| `crates/temm1e-tools/src/desktop_tool.rs` | temm1e-tools | Desktop Tool impl |

### Modified Files

| File | Change |
|------|--------|
| `Cargo.toml` (root) | Add workspace member, feature flag, optional dep |
| `crates/temm1e-core/src/traits/mod.rs` | Add `pub mod screen;` |
| `crates/temm1e-core/src/types/config.rs` | Add `GazeConfig`, `GazeBrowserConfig` |
| `crates/temm1e-core/src/types/mod.rs` | Export new types (Screenshot, Rect, etc.) |
| `crates/temm1e-tools/src/lib.rs` | Add `pub mod grounding;`, feature-gated `desktop_tool` |
| `crates/temm1e-tools/src/browser.rs` | Add `zoom_region` action, SoM overlay in Tier 3 |
| `crates/temm1e-tools/src/browser_observation.rs` | Integration point for SoM in Tier 3 selection |
| `crates/temm1e-tools/src/prowl_blueprints.rs` | Add `try_blueprint_bypass()` |
| `crates/temm1e-tools/Cargo.toml` | Add `temm1e-gaze` optional dep |
| `src/main.rs` | Register desktop tool when enabled |
