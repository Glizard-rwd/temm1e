//! Prowl Web Blueprints — pre-authored procedural blueprints for common web tasks.
//!
//! These are static blueprint documents (YAML frontmatter + Markdown body) that
//! get seeded into memory on first Prowl-enabled run. The agent's blueprint
//! system matches them via `semantic_tags` when the classifier detects a web task.

pub mod login_registry;

/// Result of a blueprint bypass check.
///
/// When a URL matches a known service in the login registry, the system can
/// skip the full vision grounding pipeline and use known CSS selectors directly.
#[derive(Debug, Clone)]
pub enum BlueprintAction {
    /// Click the element matching this CSS selector.
    ClickSelector(String),
    /// Type text into the element matching this CSS selector.
    TypeInSelector { selector: String, text: String },
    /// The URL matched a known service — the login blueprint should be used.
    /// Contains the service name and login URL for blueprint routing.
    UseLoginBlueprint { service: String, login_url: String },
}

/// Check if a URL matches a known Prowl blueprint, enabling bypass of the
/// full vision grounding pipeline.
///
/// Currently checks against the login registry (100+ services). If the URL's
/// domain matches a known service, returns a `BlueprintAction::UseLoginBlueprint`
/// with the service name and canonical login URL.
///
/// Returns `None` if no blueprint matches — the caller should fall through to
/// normal vision grounding.
///
/// # Arguments
/// - `url` — the current page URL
/// - `_intent` — the user's intent description (reserved for future semantic matching)
pub fn try_blueprint_bypass(url: &str, _intent: &str) -> Option<BlueprintAction> {
    // Extract domain from the URL for matching
    let domain = login_registry::extract_domain(url)?;
    let domain_lower = domain.to_lowercase();

    // Check if this domain matches any known service in the login registry.
    // We iterate the registry to find a domain match (login URLs contain the domain).
    // The registry maps service_name → login_url, so we check if any login URL
    // shares the same domain as the current URL.
    for service_name in login_registry::known_service_names() {
        if let Some(login_url) = login_registry::lookup_login_url(service_name) {
            if let Some(service_domain) = login_registry::extract_domain(login_url) {
                if service_domain.to_lowercase() == domain_lower {
                    return Some(BlueprintAction::UseLoginBlueprint {
                        service: service_name.to_string(),
                        login_url: login_url.to_string(),
                    });
                }
            }
        }
    }

    None
}

/// Pre-authored web blueprints for Tem Prowl.
///
/// Each entry is `(blueprint_id, blueprint_content)` where the content is a
/// YAML+Markdown document compatible with `parse_blueprint()` in `temm1e-agent`.
///
/// Seed these into memory during agent initialization when the browser tool is
/// enabled, using `MemoryEntryType::Blueprint`.
pub const WEB_BLUEPRINTS: &[(&str, &str)] = &[
    (
        "bp_prowl_search",
        include_str!("prowl_blueprints/web_search.md"),
    ),
    (
        "bp_prowl_login",
        include_str!("prowl_blueprints/web_login.md"),
    ),
    (
        "bp_prowl_extract",
        include_str!("prowl_blueprints/web_extract.md"),
    ),
    (
        "bp_prowl_compare",
        include_str!("prowl_blueprints/web_compare.md"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_blueprints_have_yaml_frontmatter() {
        for (id, content) in WEB_BLUEPRINTS {
            let trimmed = content.trim();
            assert!(
                trimmed.starts_with("---"),
                "Blueprint {id} missing YAML frontmatter opening"
            );
            let after_opening = trimmed[3..].trim_start_matches(['\r', '\n']);
            assert!(
                after_opening.contains("\n---"),
                "Blueprint {id} missing YAML frontmatter closing"
            );
        }
    }

    #[test]
    fn all_blueprints_contain_expected_id() {
        for (id, content) in WEB_BLUEPRINTS {
            assert!(
                content.contains(&format!("id: {id}")),
                "Blueprint {id} does not contain its expected id in frontmatter"
            );
        }
    }

    #[test]
    fn all_blueprints_have_phases() {
        for (id, content) in WEB_BLUEPRINTS {
            assert!(
                content.contains("## Phases"),
                "Blueprint {id} missing ## Phases section"
            );
        }
    }

    #[test]
    fn blueprint_count() {
        assert_eq!(WEB_BLUEPRINTS.len(), 4);
    }

    #[test]
    fn blueprint_bypass_known_service() {
        let result = try_blueprint_bypass("https://www.facebook.com/profile", "click login");
        assert!(result.is_some(), "Facebook URL should match a blueprint");
        match result.unwrap() {
            BlueprintAction::UseLoginBlueprint { service, .. } => {
                assert!(
                    service == "facebook" || service == "fb",
                    "Service should be facebook or fb, got: {service}"
                );
            }
            other => panic!("Expected UseLoginBlueprint, got: {other:?}"),
        }
    }

    #[test]
    fn blueprint_bypass_unknown_site() {
        let result = try_blueprint_bypass("https://some-random-site.com/page", "click the button");
        assert!(
            result.is_none(),
            "Unknown URL should not match any blueprint"
        );
    }

    #[test]
    fn blueprint_bypass_invalid_url() {
        let result = try_blueprint_bypass("not-a-url", "click something");
        assert!(result.is_none(), "Invalid URL should return None");
    }
}
