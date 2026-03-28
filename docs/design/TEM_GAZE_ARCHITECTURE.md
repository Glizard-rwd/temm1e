# Tem Gaze — Desktop Vision Control Architecture

> **Status:** Design Complete
> **Scope:** Browser vision upgrade (Prowl V2) + Full desktop control (Ubuntu, macOS)
> **Lab:** [tems_lab/gaze/](../../tems_lab/gaze/)

---

## What Is Tem Gaze?

Tem Gaze extends TEMM1E from browser-only control (Tem Prowl) to full computer control. It lets Tem see the screen, identify UI elements, and click precisely — on any application, any platform.

**Browser (Prowl V2):** Enhanced vision grounding with zoom-refine, Set-of-Mark overlays, and blueprint bypass. DOM remains primary; vision is the accuracy enhancer.

**Desktop (Tem Gaze):** Vision-primary control using the user's configured VLM. No DOM, no mandatory accessibility tree, no local detection models. The VLM sees screenshots and determines where to click.

---

## Architecture

```
                        ┌─────────────────────────────┐
                        │    ScreenController trait     │
                        │    (temm1e-core/traits/)      │
                        │                               │
                        │  capture() → Screenshot       │
                        │  click(x, y)                  │
                        │  type_text(s)                 │
                        │  key_combo(keys)              │
                        │  scroll(x, y, dir, amount)    │
                        │  drag(x1, y1, x2, y2)        │
                        │  accessibility_tree() → Opt   │
                        └───────────┬───────────────────┘
                                    │
                 ┌──────────────────┼──────────────────┐
                 │                  │                   │
        BrowserController   DesktopController    (Future: Remote)
        (temm1e-tools)      (temm1e-gaze)        (VNC/RDP)
        CDP commands        enigo + xcap          Network protocol
        DOM: 100%           A11y: optional         A11y: none
        Existing            Feature-gated          Future work
```

### Grounding Pipeline (Shared)

```
User intent + Screenshot
    │
    ├── Browser + Blueprint match? → Known selectors (0 LLM calls)
    ├── Browser + A11y tree?       → SoM overlay + select [N] (1 LLM call)
    │
    └── Vision grounding (browser fallback or desktop primary):
        │
        ├── Coarse pass → HIGH confidence → Direct click (1 call)
        ├── Coarse pass → MED confidence  → Zoom-refine (2 calls)
        └── Coarse pass → LOW confidence  → Zoom + SoM (3 calls)
                                               │
                                    Optional: Verify-retry loop (+1 call)
```

---

## Key Design Decisions

### 1. Vision-Primary for Desktop

Desktop accessibility coverage is 33-65% (vs browser DOM's 100%). Industry convergence: Claude Computer Use, UI-TARS, Agent S2, AskUI, Fara-7B all use pure vision. Vision is primary; accessibility is an optional cost optimizer.

### 2. Single Model, Always

All grounding calls use the user's configured provider and model. No multi-model routing, no cross-provider grounding. Works identically for Anthropic, OpenAI, Gemini, OpenRouter, custom proxies, and local Ollama.

### 3. Zero New Dependencies for Browser Users

Desktop control is feature-gated (`--features desktop-control`). Browser-only users see zero new dependencies, zero binary size increase, zero behavior change.

### 4. Model-Agnostic Techniques

Zoom-refine, SoM overlay, and verify-retry improve accuracy regardless of which VLM runs them. The pipeline automatically improves as VLMs improve.

---

## Crate Map

```
crates/
  temm1e-core/
    src/traits/screen.rs     ← NEW: ScreenController trait
    src/types/config.rs      ← MODIFIED: GazeConfig section

  temm1e-gaze/               ← NEW CRATE (feature-gated)
    src/
      desktop_controller.rs  -- DesktopScreenController (enigo + xcap)
      overlay.rs             -- SoM label compositing on screenshots
      coordinate.rs          -- DPI scaling, API resolution transforms
      platform/
        macos.rs             -- Retina handling, key mapping
        linux.rs             -- X11/Wayland detection, key mapping

  temm1e-tools/
    src/
      grounding.rs           ← NEW: Shared grounding pipeline
      desktop_tool.rs        ← NEW: Desktop Tool (feature-gated)
      browser.rs             ← MODIFIED: zoom_region action, SoM in Tier 3
      browser_observation.rs ← MODIFIED: SoM integration point
      prowl_blueprints.rs    ← MODIFIED: try_blueprint_bypass()
```

---

## Configuration

```toml
# skyclaw.toml

[gaze]
enabled = false              # Enable desktop control tool
high_confidence = 0.85       # Direct click threshold
medium_confidence = 0.40     # Zoom-refine threshold
verify_mode = "high_stakes"  # off | high_stakes | always
monitor = 0                  # Primary monitor
use_accessibility = false    # Optional a11y cost optimizer

[gaze.browser]
som_overlay = true           # SoM labels on Tier 3 observations
zoom_region = true           # Enable zoom_region browser action
blueprint_bypass = true      # Check known flows before vision
```

---

## Cost Profile

| Scenario | Calls | Est. Cost (Sonnet) |
|----------|-------|--------------------|
| Blueprint bypass (known login flow) | 0 | $0.000 |
| Browser SoM (a11y available) | 1 | ~$0.003 |
| Easy desktop target (high conf) | 1 | ~$0.003 |
| Medium desktop target (zoom-refine) | 2 | ~$0.006 |
| Hard desktop target (zoom + SoM) | 3 | ~$0.009 |
| With verification | +1 | +$0.003 |

Compare: Claude Computer Use sends a full screenshot (~$0.003-0.005) on every step regardless of difficulty. Tem Gaze's adaptive pipeline is cheaper for easy targets and comparable for hard ones.

---

## Platform Support

| Platform | Screen Capture | Input Simulation | A11y (optional) |
|----------|---------------|------------------|-----------------|
| macOS (ARM + Intel) | xcap (Core Graphics) | enigo (CGEventPost) | AX API (feature-gated) |
| Ubuntu X11 | xcap (XGetImage) | enigo (XTEST) | AT-SPI2 (feature-gated) |
| Ubuntu Wayland | xcap (PipeWire portal) | enigo (experimental) | AT-SPI2 read-only |
| Browser (all platforms) | CDP screenshot | CDP mouse/keyboard | CDP accessibility tree |

---

## Naming Lineage

```
Tem Prowl   → Hunts the web (browser control)
Tem Gaze    → Sees and commands the machine (desktop control)
Tem Hive    → Swarms together (multi-agent)
Eigen-Tune  → Sharpens itself (self-tuning)
```

Prowl navigates. Gaze targets. Together they give Tem full perception and control across browser and desktop.
