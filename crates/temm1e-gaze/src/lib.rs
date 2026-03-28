//! Tem Gaze — vision-primary desktop control for TEMM1E.
//!
//! Provides OS-level screen capture and input simulation for full computer
//! control. Uses `xcap` for cross-platform screen capture and `enigo` for
//! mouse/keyboard input simulation.
//!
//! ## macOS Permission
//!
//! Input simulation on macOS requires the Accessibility permission.
//! Grant it in System Settings → Privacy & Security → Accessibility
//! for the terminal or binary running TEMM1E.

pub mod desktop_controller;
pub mod overlay;
pub mod platform;

pub use desktop_controller::DesktopController;
