# Tem Gaze: Vision-Primary Desktop Control

## Zero-Risk Design Document v1.0

**Date:** 2026-03-28
**Status:** Design Complete, Implementation Ready
**Predecessor:** Tem Prowl (browser, DOM-primary)
**Research:** [RESEARCH_PAPER.md](RESEARCH_PAPER.md)

---

## 0. Design Philosophy

This document specifies Tem Gaze — a vision-primary desktop control system that extends TEMM1E from browser-only (Tem Prowl) to full computer control (Ubuntu + macOS). Tem Gaze uses the user's already-configured VLM provider with no additional model dependencies, no Python, and no model weight downloads.

**Core principle:** Tem Gaze is a new leaf crate (`temm1e-gaze`) that depends only on `temm1e-core`. It does not modify any existing crate's runtime logic. Integration with the agent runtime happens through the existing `Tool` trait. When the `desktop-control` feature flag is disabled (the default), the system is byte-identical to pre-Gaze TEMM1E.

**What we're building:** A ScreenController abstraction that unifies browser and desktop control behind one interface, plus a vision grounding pipeline (zoom-refine, SoM, verify-retry) shared by both browser and desktop contexts.

**What we're NOT building:**
- Local detection models (YOLO, OmniParser) — VLM-native detection is sufficient
- Multi-model routing — user's configured model handles everything
- Cross-provider grounding — single active provider, always
- Windows support (future work, separate from initial release)
- Mobile device control (different domain)

**The bet:** VLMs are rapidly improving at visual grounding (0.8% → 74% on ScreenSpot-Pro in one year). The grounding techniques (zoom-refine, SoM) are force multipliers that improve every VLM equally. By building model-agnostic techniques rather than model-specific optimizations, Tem Gaze automatically improves as VLMs improve.

---

## 1. System Axioms

These seven invariants are non-negotiable. Every mechanism must preserve all seven.

**A1 — Single Model, Always.**
All grounding calls use the user's configured provider and model. No automatic model selection, no cross-provider routing, no tier downgrading. The user's model is the only model. This applies regardless of grounding difficulty.

**A2 — Zero New Dependencies for Existing Users.**
The `desktop-control` feature flag gates all desktop-specific dependencies (`enigo`, `xcap`). Users who only use browser control see zero new dependencies, zero new binary size, zero behavior change.

**A3 — Provider Agnosticism.**
The grounding pipeline makes zero assumptions about the VLM provider. It works identically for Anthropic, OpenAI, Gemini, OpenRouter, custom proxies, and local Ollama. It requires only: (a) the model accepts image input, (b) the model returns structured text output.

**A4 — Additive Integration.**
No existing crate logic is modified. The ScreenController trait is new in `temm1e-core/traits/`. The `temm1e-gaze` crate is new. The grounding pipeline is a new module in `temm1e-tools`. Browser control continues to work exactly as before — Prowl V2 enhancements are additive.

**A5 — Vision Primary, A11y Optional.**
Desktop grounding uses vision as the primary perception mode. Accessibility APIs (AT-SPI2, macOS AX) are available as a feature-gated cost optimizer, off by default. The system must function correctly with zero accessibility data.

**A6 — Cross-Platform Parity.**
Every capability must work on both Ubuntu (X11 and Wayland) and macOS (ARM and Intel). Platform-specific code uses `#[cfg(target_os)]` with implementations for both. No Unix-only APIs without macOS equivalents and vice versa.

**A7 — Resilience Inheritance.**
All existing resilience guarantees (catch_unwind, session rollback, dead worker detection, UTF-8 safe truncation) apply to desktop control tasks. A misclick or grounding failure must not crash the worker, corrupt session state, or leave phantom input events.

---

## 2. Architecture

### 2.1 Crate Dependency Graph (After Gaze)

```
temm1e-core (traits: ScreenController, Tool, Provider, Channel, Memory, ...)
    │
    ├── temm1e-gaze (DesktopController: enigo + xcap)
    │       depends on: temm1e-core
    │       feature-gated: desktop-control
    │
    ├── temm1e-tools (BrowserController via CDP, + grounding pipeline)
    │       depends on: temm1e-core
    │       contains: browser.rs, grounding.rs (shared), desktop_tool.rs (new)
    │
    └── temm1e-agent (runtime, context, budget — unchanged)
            depends on: temm1e-core, temm1e-tools
```

### 2.2 ScreenController Trait

```rust
/// Unified interface for controlling a screen — browser or desktop.
/// Defined in temm1e-core/src/traits/screen.rs
#[async_trait]
pub trait ScreenController: Send + Sync {
    /// Capture the current screen as a PNG image.
    async fn capture(&self) -> Result<Screenshot, Temm1eError>;

    /// Move mouse and click at (x, y).
    async fn click(&self, x: u32, y: u32) -> Result<(), Temm1eError>;

    /// Double-click at (x, y).
    async fn double_click(&self, x: u32, y: u32) -> Result<(), Temm1eError>;

    /// Right-click at (x, y).
    async fn right_click(&self, x: u32, y: u32) -> Result<(), Temm1eError>;

    /// Type text string (supports Unicode).
    async fn type_text(&self, text: &str) -> Result<(), Temm1eError>;

    /// Press key combination (e.g., &["ctrl", "c"]).
    async fn key_combo(&self, keys: &[&str]) -> Result<(), Temm1eError>;

    /// Scroll at position (x, y) in given direction.
    async fn scroll(
        &self, x: u32, y: u32, direction: ScrollDirection, amount: u32
    ) -> Result<(), Temm1eError>;

    /// Drag from (x1, y1) to (x2, y2).
    async fn drag(
        &self, x1: u32, y1: u32, x2: u32, y2: u32
    ) -> Result<(), Temm1eError>;

    /// Get screen dimensions (width, height) in logical coordinates.
    fn dimensions(&self) -> (u32, u32);

    /// Get DPI scale factor (1.0 for standard, 2.0 for Retina).
    fn scale_factor(&self) -> f64;

    /// Optional: get accessibility tree if available.
    /// Returns None if a11y is unavailable or disabled.
    async fn accessibility_tree(&self) -> Option<Vec<A11yElement>>;
}

pub struct Screenshot {
    pub data: Vec<u8>,      // PNG bytes
    pub width: u32,         // Logical width
    pub height: u32,        // Logical height
    pub scale_factor: f64,  // DPI scale
}

pub struct A11yElement {
    pub role: String,       // "button", "textbox", "link", etc.
    pub name: String,       // Label text
    pub bounds: Rect,       // Bounding box in logical coordinates
    pub interactive: bool,  // Whether clickable/typeable
}

pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub enum ScrollDirection {
    Up, Down, Left, Right,
}
```

### 2.3 Implementations

**BrowserScreenController** (in `temm1e-tools`):
- `capture()` → CDP `Page::screenshot()`
- `click()` → CDP `Input.dispatchMouseEvent`
- `type_text()` → CDP `Input.dispatchKeyEvent`
- `accessibility_tree()` → CDP `Accessibility.getFullAXTree` + JS `getBoundingClientRect()`
- Already implemented in browser.rs — wrap existing functions behind the trait

**DesktopScreenController** (in `temm1e-gaze`):
- `capture()` → `xcap::Monitor::capture_image()`
- `click()` → `enigo::Mouse::move_to()` + `enigo::Mouse::click()`
- `type_text()` → `enigo::Keyboard::text()`
- `key_combo()` → `enigo::Keyboard::key()` with modifiers
- `accessibility_tree()` → AT-SPI2 via `atspi` crate (Linux) or AX API FFI (macOS), feature-gated

### 2.4 Desktop Tool Declaration

```rust
/// New tool in temm1e-tools, registered alongside browser tool
ToolDeclaration {
    name: "desktop",
    description: "Control the computer desktop — click, type, scroll,
                  capture screenshots. Works on Ubuntu and macOS.",
    parameters: {
        "action": enum [
            "screenshot",       // Capture current screen
            "click",            // Click at coordinates
            "double_click",     // Double-click at coordinates
            "right_click",      // Right-click at coordinates
            "type",             // Type text (optionally at coordinates)
            "key",              // Press key combination
            "scroll",           // Scroll at coordinates
            "drag",             // Drag between coordinates
            "observe",          // Capture + grounding analysis
            "zoom_region",      // Crop region for detailed analysis
        ],
        "x": "number — X coordinate (logical pixels)",
        "y": "number — Y coordinate (logical pixels)",
        "x2": "number — End X for drag",
        "y2": "number — End Y for drag",
        "text": "string — Text to type or key combo (e.g., 'ctrl+c')",
        "direction": "enum [up, down, left, right] — Scroll direction",
        "amount": "number — Scroll amount (default 3)",
        "region": "[x1, y1, x2, y2] — Region for zoom_region",
    }
}
```

---

## 3. Vision Grounding Pipeline

### 3.1 Pipeline Architecture

The grounding pipeline is shared between browser and desktop contexts. It lives in `temm1e-tools/src/grounding.rs`.

```
User intent + Screenshot
    │
    ▼
┌──────────────────────────────────────────────────┐
│  CONTEXT CHECK                                    │
│                                                    │
│  Browser context?                                  │
│    → Has a11y tree with bounding boxes?            │
│      → Yes: Use SoM overlay directly (cheapest)   │
│      → No: Fall through to vision grounding        │
│                                                    │
│  Prowl blueprint match?                            │
│    → Yes: Use known selectors (0 LLM calls)       │
│    → No: Fall through to vision grounding          │
│                                                    │
│  Desktop context?                                  │
│    → Always: vision grounding pipeline             │
│    → Optional: a11y augmentation if enabled        │
└──────────────────────┬───────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────┐
│  VISION GROUNDING (model-agnostic)                │
│                                                    │
│  Step 1: Coarse Pass                               │
│    Send screenshot + intent to user's model        │
│    Receive: target description, confidence,        │
│             bounding region [x1,y1,x2,y2]          │
│                                                    │
│  Step 2: Confidence Gate                           │
│    HIGH (≥0.85): Direct click at centroid → done   │
│    MED (0.4-0.85): Proceed to zoom-refine          │
│    LOW (<0.4): Proceed to zoom-refine + SoM        │
│                                                    │
│  Step 3: Zoom-Refine (if needed)                   │
│    Crop screenshot to bounding region              │
│    Resize to fill API resolution constraint        │
│    Re-send to model for precise coordinates        │
│    Transform coordinates back to original space    │
│                                                    │
│  Step 4: SoM Fallback (if still low confidence)   │
│    Overlay numbered labels on zoomed region        │
│    Re-send: "select element [N]"                   │
│    Look up element N's centroid                    │
│                                                    │
│  Step 5: Verify (optional, for high-stakes)        │
│    Capture post-action screenshot                  │
│    Compare before/after with model                 │
│    Retry if expected change didn't occur           │
└──────────────────────────────────────────────────┘
```

### 3.2 Coordinate Transform Mathematics

**Zoom-refine coordinate transform:**

Given:
- Original screenshot dimensions: `(W, H)`
- Coarse bounding region: `(x1, y1, x2, y2)` in original space
- Cropped region dimensions: `(w_crop, h_crop)` where `w_crop = x2 - x1`, `h_crop = y2 - y1`
- VLM returns coordinates `(x_zoomed, y_zoomed)` in the zoomed image space
- Zoomed image was resized from `(w_crop, h_crop)` to `(W_api, H_api)` for the API

Transform back to original coordinates:

```
x_original = x1 + (x_zoomed / W_api) * w_crop
y_original = y1 + (y_zoomed / H_api) * h_crop
```

**DPI scaling transform:**

Given:
- Screen physical resolution: `(W_phys, H_phys)` (e.g., 2560x1600 for Retina)
- Scale factor: `s` (e.g., 2.0 for Retina)
- Logical resolution: `(W_log, H_log)` = `(W_phys/s, H_phys/s)` (e.g., 1280x800)
- Screenshot captured at physical resolution by `xcap`
- VLM returns coordinates in screenshot space (physical pixels)

Transform to logical coordinates for input simulation:

```
x_logical = x_physical / s
y_logical = y_physical / s
```

**API resolution constraint scaling:**

Given:
- Screenshot dimensions: `(W_src, H_src)`
- API max longest edge: `L_max` (e.g., 1568 for Anthropic)
- API max total pixels: `P_max` (e.g., 1,150,000 for Anthropic)

Compute scale factor:

```
s_edge = min(1.0, L_max / max(W_src, H_src))
s_pixels = min(1.0, sqrt(P_max / (W_src * H_src)))
s = min(s_edge, s_pixels)

W_api = round(W_src * s)
H_api = round(H_src * s)
```

Coordinates returned by VLM are in `(W_api, H_api)` space. Scale back:

```
x_screen = x_api / s
y_screen = y_api / s
```

### 3.3 Confidence Thresholds

| Threshold | Default | Purpose | Tuning Source |
|-----------|---------|---------|---------------|
| `HIGH_CONF` | 0.85 | Skip zoom-refine, direct click | Eigen-Tune (future) |
| `MED_CONF` | 0.40 | Skip SoM, use zoom-refine only | Eigen-Tune (future) |
| `VERIFY_THRESHOLD` | 0.70 | Accept verification as passing | Eigen-Tune (future) |
| `MAX_RETRIES` | 2 | Max retry attempts after verify failure | Static config |
| `UI_SETTLE_MS` | 500 | Wait after action for UI to update | Per-platform default |

These are configured in `skyclaw.toml`:

```toml
[gaze]
# Vision grounding thresholds
high_confidence = 0.85
medium_confidence = 0.40
verify_threshold = 0.70
max_retries = 2
ui_settle_ms = 500

# Optional accessibility (off by default)
use_accessibility = false

# Verification mode: "off", "high_stakes", "always"
verify_mode = "high_stakes"
```

### 3.4 Grounding Prompt Templates

**Coarse Pass Prompt:**
```
Analyze this screenshot and identify the UI element needed for the following action:
"{user_intent}"

Return a JSON object:
{
  "description": "Brief description of the target element",
  "confidence": 0.0-1.0,
  "region": [x1, y1, x2, y2],
  "coordinates": [x, y]
}

Where region is the bounding box around the target area, and coordinates
is your best estimate of the exact click point. Both use pixel coordinates
relative to this image (width={W}, height={H}).
```

**Zoom-Refine Prompt:**
```
This is a zoomed-in view of a region from the desktop screenshot.
Identify the exact element needed for: "{user_intent}"

Return a JSON object:
{
  "description": "The specific element to click",
  "confidence": 0.0-1.0,
  "coordinates": [x, y]
}

Coordinates are in pixels relative to THIS image (width={W}, height={H}).
```

**SoM Selection Prompt:**
```
This image shows numbered interactive elements in the target region.
Select the element needed for: "{user_intent}"

Return a JSON object:
{
  "element": N,
  "confidence": 0.0-1.0,
  "reasoning": "Why this element"
}
```

**Verification Prompt:**
```
Compare these two screenshots (before and after an action).
The intended action was: "{user_intent}"
The action performed was: click at ({x}, {y})

Did the expected change occur?
Return a JSON object:
{
  "success": true/false,
  "confidence": 0.0-1.0,
  "actual_change": "Description of what changed between screenshots"
}
```

---

## 4. SoM Overlay Implementation

### 4.1 Browser Context (Prowl V2)

TEMM1E already has numbered overlay injection in `browser_session.rs` (`OVERLAY_INJECT_JS`). Prowl V2 generalizes this:

```javascript
// Inject into any page via CDP, not just OTK sessions
// For each interactive element with known bounding box:
function overlayElement(index, x, y, width, height) {
    const label = document.createElement('div');
    label.style.cssText = `
        position: fixed;
        left: ${x}px; top: ${y}px;
        width: 22px; height: 22px;
        background: #e53e3e;
        color: white;
        border-radius: 50%;
        display: flex; align-items: center; justify-content: center;
        font: bold 11px/1 monospace;
        z-index: 2147483647;
        pointer-events: none;
    `;
    label.textContent = index;
    label.dataset.gazeOverlay = 'true';
    document.body.appendChild(label);
}
```

Bounding boxes come from `getBoundingClientRect()` on each element identified in the accessibility tree — already available in Tier 1 observation.

### 4.2 Desktop Context

For desktop, there's no DOM to inject overlays into. Instead, overlay is composited onto the screenshot image before sending to the VLM:

```rust
/// Composite numbered labels onto a screenshot PNG
/// in temm1e-gaze/src/overlay.rs
fn overlay_som_labels(
    screenshot: &Screenshot,
    elements: &[GroundingCandidate],
) -> Vec<u8> {
    // 1. Decode PNG to RGBA buffer
    // 2. For each candidate element:
    //    - Draw red circle at element's estimated position
    //    - Render white number inside circle
    // 3. Re-encode as PNG
    // Uses the `image` crate (already a transitive dependency)
}
```

Element positions for desktop SoM come from the VLM's coarse pass — the zoom-refine step identifies multiple candidate regions, and SoM labels them for final selection.

---

## 5. Prowl V2: Browser Vision Upgrade

Prowl V2 enhances the existing browser tool with grounding techniques. No new crates — all changes are within `temm1e-tools`.

### 5.1 New Browser Actions

**`zoom_region`** — Crop a region of the current page screenshot and return at full resolution:

```rust
// In browser.rs, alongside existing actions
"zoom_region" => {
    let x1 = params.get("x1").as_u32()?;
    let y1 = params.get("y1").as_u32()?;
    let x2 = params.get("x2").as_u32()?;
    let y2 = params.get("y2").as_u32()?;

    // Capture full screenshot
    let screenshot = page.screenshot(/* ... */).await?;

    // Crop to region, resize to API max resolution
    let cropped = crop_and_resize(&screenshot, x1, y1, x2, y2)?;

    // Store as last_image for vision pipeline injection
    self.last_image = Some(ToolOutputImage {
        media_type: "image/png".into(),
        data: base64::encode(&cropped),
    });

    Ok(ToolOutput::text(format!(
        "Zoomed into region ({x1},{y1})-({x2},{y2}).
         Analyze this zoomed view for more detail."
    )))
}
```

### 5.2 Generalized SoM Overlay

Currently, numbered overlays only appear in `browser_session.rs` for OTK interactive login. Prowl V2 extends this to `browser_observation.rs` Tier 3:

When `select_tier()` returns Tier 3 (screenshot needed), also inject SoM overlays on interactive elements:

```
Tier 3 observation flow (updated):
1. Capture accessibility tree
2. Get bounding boxes via getBoundingClientRect() for interactive elements
3. Inject SoM overlay labels onto page
4. Capture screenshot (now includes overlays)
5. Remove overlay labels from page
6. Return: accessibility tree text + annotated screenshot
```

The VLM receives both the tree (with [N] indexed elements) and the screenshot (with matching [N] labels visible). It can reference elements by number instead of guessing coordinates.

### 5.3 Blueprint Bypass Enhancement

Prowl blueprints (`prowl_blueprints.rs`) already define known flows for login, search, extract, and compare. Enhancement: check blueprint match BEFORE entering the vision pipeline.

```
Incoming browser action:
  → Match against blueprint registry (pattern match on URL + intent)
  → Match found? Execute blueprint selectors directly (0 VLM calls)
  → No match? Proceed to normal grounding pipeline
```

This is particularly powerful for the login registry (100+ services with known URLs and selectors).

---

## 6. Tem Gaze: Desktop Control

### 6.1 Crate Structure

```
crates/temm1e-gaze/
    Cargo.toml
    src/
        lib.rs                  -- Crate root, re-exports
        desktop_controller.rs   -- DesktopScreenController (enigo + xcap)
        overlay.rs              -- Screenshot SoM overlay compositing
        coordinate.rs           -- DPI scaling, API resolution transforms
        platform/
            mod.rs
            macos.rs            -- macOS-specific: Retina handling, key mapping
            linux.rs            -- Linux-specific: X11/Wayland detection, key mapping
```

### 6.2 Dependencies

```toml
[package]
name = "temm1e-gaze"
version = "0.1.0"
edition = "2021"

[dependencies]
temm1e-core = { path = "../temm1e-core" }
async-trait = "0.1"
enigo = { version = "0.3", features = ["serde"] }
xcap = "0.8"
image = "0.25"    # For screenshot manipulation and SoM overlay
tracing = "0.1"
tokio = { version = "1", features = ["time"] }

[target.'cfg(target_os = "linux")'.dependencies]
# Optional: AT-SPI2 for accessibility acceleration
atspi = { version = "0.29", optional = true }

[target.'cfg(target_os = "macos")'.dependencies]
# Optional: macOS accessibility
accessibility-sys = { version = "0.2", optional = true }

[features]
default = []
accessibility = ["atspi", "accessibility-sys"]
```

### 6.3 DesktopScreenController Implementation

```rust
// crates/temm1e-gaze/src/desktop_controller.rs

pub struct DesktopScreenController {
    enigo: Mutex<Enigo>,
    monitor_index: usize,  // Which monitor to control (default: primary)
}

#[async_trait]
impl ScreenController for DesktopScreenController {
    async fn capture(&self) -> Result<Screenshot, Temm1eError> {
        let monitors = xcap::Monitor::all()
            .map_err(|e| Temm1eError::Tool(format!("Screen capture failed: {e}")))?;
        let monitor = monitors.get(self.monitor_index)
            .ok_or_else(|| Temm1eError::Tool("Monitor not found".into()))?;
        let image = monitor.capture_image()
            .map_err(|e| Temm1eError::Tool(format!("Capture failed: {e}")))?;

        let mut png_bytes = Vec::new();
        image.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)?;

        let scale = monitor.scale_factor();
        Ok(Screenshot {
            data: png_bytes,
            width: (image.width() as f64 / scale) as u32,
            height: (image.height() as f64 / scale) as u32,
            scale_factor: scale,
        })
    }

    async fn click(&self, x: u32, y: u32) -> Result<(), Temm1eError> {
        let mut enigo = self.enigo.lock().await;
        enigo.move_mouse(x as i32, y as i32, Coordinate::Abs)
            .map_err(|e| Temm1eError::Tool(format!("Mouse move failed: {e}")))?;
        // Small delay for move to register
        tokio::time::sleep(Duration::from_millis(50)).await;
        enigo.button(Button::Left, Click)
            .map_err(|e| Temm1eError::Tool(format!("Click failed: {e}")))?;
        Ok(())
    }

    async fn type_text(&self, text: &str) -> Result<(), Temm1eError> {
        let mut enigo = self.enigo.lock().await;
        enigo.text(text)
            .map_err(|e| Temm1eError::Tool(format!("Type failed: {e}")))?;
        Ok(())
    }

    // ... other methods follow the same pattern

    fn dimensions(&self) -> (u32, u32) {
        // Return logical dimensions (physical / scale_factor)
    }

    fn scale_factor(&self) -> f64 {
        // Query from xcap::Monitor
    }

    async fn accessibility_tree(&self) -> Option<Vec<A11yElement>> {
        #[cfg(feature = "accessibility")]
        {
            // Platform-specific a11y tree reading
            // Returns None on failure or timeout
        }
        #[cfg(not(feature = "accessibility"))]
        { None }
    }
}
```

---

## 7. Configuration Schema

### 7.1 skyclaw.toml additions

```toml
# Desktop control configuration
[gaze]
# Enable desktop control tool
enabled = false

# Vision grounding thresholds
high_confidence = 0.85      # Direct click without zoom
medium_confidence = 0.40    # Zoom-refine without SoM
verify_threshold = 0.70     # Accept verification as passing
max_retries = 2             # Max retries after verification failure
ui_settle_ms = 500          # Wait for UI after action (ms)

# Verification mode
# "off"         - Never verify (fastest, cheapest)
# "high_stakes" - Verify destructive actions (delete, submit, send)
# "always"      - Verify every action (most reliable, most expensive)
verify_mode = "high_stakes"

# Monitor selection (0 = primary)
monitor = 0

# Use OS accessibility APIs when available (cost optimizer)
use_accessibility = false

# Prowl V2 browser vision enhancements
[gaze.browser]
# Enable SoM overlay on Tier 3 observations
som_overlay = true
# Enable zoom_region action
zoom_region = true
# Blueprint bypass (check known flows before vision)
blueprint_bypass = true
```

### 7.2 Feature Flags in Root Cargo.toml

```toml
[features]
default = []
tui = ["temm1e-tui"]
desktop-control = ["temm1e-gaze"]
desktop-accessibility = ["temm1e-gaze/accessibility"]
```

---

## 8. Risk Assessment

### 8.1 Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| VLM returns coordinates outside screen bounds | Low | Clamp to (0,0)-(W,H) before input simulation |
| DPI scale mismatch (screenshot vs input coords) | Medium | Explicit scale_factor tracking, test on Retina + non-Retina |
| Wayland screen capture permission dialog | Low | Document in setup guide, `xcap` handles portal negotiation |
| `enigo` fails on Wayland for input simulation | Medium | Detect Wayland, log warning, suggest XWayland fallback |
| Misclick on destructive element | High | verify_mode="high_stakes" default, require confirmation for delete/send |
| Phantom input events on panic | High | Guard all input simulation with catch_unwind, no partial mouse states |
| VLM hallucinates UI elements | Medium | Zoom-refine + verify-retry catches most hallucinations |
| API resolution scaling introduces rounding errors | Low | Use f64 arithmetic, round only at final pixel coordinate |
| Browser SoM overlay not cleaned up on error | Low | `defer` pattern: inject overlay, screenshot, remove overlay in finally block |
| Desktop SoM overlay bloats image size | Low | PNG compression handles small overlays efficiently |

### 8.2 Security Considerations

**Input simulation scope:** `enigo` simulates input at the OS level. A malicious VLM response could theoretically instruct Tem to type passwords, click "delete all," or navigate to harmful URLs. Mitigations:

1. **High-stakes verification:** Actions on elements matching destructive patterns (delete, remove, send, submit, purchase, sign out) require post-action verification.
2. **Input rate limiting:** Max 10 actions per second, max 100 actions per task. Prevents runaway clicking.
3. **User approval for sensitive domains:** First-time interaction with banking, email, social media prompts user confirmation via messaging channel.
4. **Screenshot evidence:** Every action's before/after screenshots are available for user review via the messaging channel.
5. **Kill switch:** User can send `/stop` via any channel to halt all desktop actions immediately, leveraging existing CancellationToken infrastructure.

---

## 9. Relationship to Existing Systems

### 9.1 Prowl → Gaze Evolution

```
Tem Prowl (v1, current)          Tem Prowl V2             Tem Gaze
────────────────────            ──────────────            ──────────
Browser only                    Browser, enhanced          Browser + Desktop
DOM-primary, vision fallback    DOM-primary + SoM/zoom    DOM for browser, vision for desktop
OTK overlays only               General SoM overlays      SoM on screenshots (image compositing)
Raw click_at(x,y)               click_at + zoom_region    ScreenController.click(x,y)
No verification                 Verify-retry loop          Verify-retry loop
4 blueprints                    Blueprint bypass first     Blueprint bypass first
```

### 9.2 Eigen-Tune Integration (Future)

Eigen-Tune (`temm1e-distill`) will track grounding outcomes:

```rust
// Future: GroundingOutcome fed to Eigen-Tune
struct GroundingOutcome {
    context: GroundingContext,       // Browser or Desktop
    strategy: GroundingStrategy,    // DirectClick, ZoomRefine, ZoomSoM
    element_density: u32,           // Approx elements on screen
    succeeded: bool,                // Did verification pass?
    cost_tokens: u32,               // Tokens consumed
    latency_ms: u32,                // Wall-clock time
}
```

Over time, Eigen-Tune adjusts confidence thresholds:
- "For screens with >50 visible elements, lower HIGH_CONF to 0.75 (more zoom-refine improves accuracy)"
- "For text editors, raise HIGH_CONF to 0.95 (large targets, zoom-refine wastes tokens)"

This is pure statistical threshold tuning in SQLite. No model weights, no GPU, no Python.

### 9.3 Hive Swarm Integration

Each Hive agent operating independently benefits from improved grounding. No special "Hive Vision" mode — each agent has its own screen context (browser via browser_pool, desktop via separate DesktopScreenController instances) and runs the grounding pipeline independently.

---

## 10. Testing Strategy

### 10.1 Unit Tests

| Component | Test | Location |
|-----------|------|----------|
| Coordinate transforms | DPI scaling, zoom-refine math, API resolution scaling | `temm1e-gaze/src/coordinate.rs` |
| SoM overlay compositing | Overlay renders correctly, doesn't corrupt PNG | `temm1e-gaze/src/overlay.rs` |
| Confidence thresholds | Correct routing: high→direct, med→zoom, low→SoM | `temm1e-tools/src/grounding.rs` |
| Config parsing | `[gaze]` section parses correctly | `temm1e-core/src/types/config.rs` |
| Prompt templates | Templates render with correct dimensions and intent | `temm1e-tools/src/grounding.rs` |
| Key mapping | Platform key names map correctly | `temm1e-gaze/src/platform/` |

### 10.2 Integration Tests

| Test | Description |
|------|-------------|
| Browser SoM overlay | Inject overlay, capture screenshot, verify labels visible, clean up |
| Browser zoom_region | Navigate to page, zoom into region, verify cropped image dimensions |
| Grounding pipeline mock | Mock VLM responses, verify correct strategy selection and coordinate transforms |
| Desktop capture | Capture screenshot, verify dimensions match expected resolution |
| Desktop input | Click, type, verify via screenshot comparison (requires display) |

### 10.3 Live Validation

Following tems_lab convention, live benchmarks will be conducted after implementation:
- Grounding accuracy on captured desktop screenshots (manual annotation)
- Token cost per grounding across difficulty levels
- Action success rate on scripted desktop tasks
- Comparison: raw click_at vs zoom-refine vs zoom+SoM

---

## 11. Non-Goals (Explicit)

1. **No local detection models.** No YOLO, OmniParser, or any model that requires separate weight downloads. The VLM IS the detector.
2. **No multi-model routing.** User's model handles everything. No automatic tier selection.
3. **No Windows support in v1.** Windows requires UIA bindings and is a separate effort.
4. **No mobile device control.** Android/iOS are separate domains with different abstractions.
5. **No real-time screen streaming.** Gaze captures discrete screenshots, not video. Frame-by-frame is sufficient for agent tasks.
6. **No game control.** Gaming UIs have sub-frame timing requirements beyond agent-loop latency.
