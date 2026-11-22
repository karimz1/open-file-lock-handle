//! The `gui-e2e` feature embeds a WebDriver server that lets any local process drive
//! the app. It must stay a test-only build: never a default feature and never used by
//! a workflow that builds or ships release artifacts.
use std::fs;
use std::path::Path;

const FEATURE: &str = "gui-e2e";
/// The only workflow allowed to build with the feature.
const TEST_WORKFLOW: &str = "gui-e2e.yml";

#[test]
fn gui_e2e_is_not_a_default_feature() {
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let default = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("default ="))
        .expect("Cargo.toml declares default features");
    assert!(
        !default.contains(FEATURE),
        "{FEATURE} must not be a default feature: {default}"
    );
}

#[test]
fn only_the_gui_e2e_workflow_builds_with_the_webdriver_feature() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    let mut checked = 0;
    for entry in fs::read_dir(&workflows).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !(name.ends_with(".yml") || name.ends_with(".yaml")) || name == TEST_WORKFLOW {
            continue;
        }
        checked += 1;
        let text = fs::read_to_string(&path).unwrap();
        for (number, line) in text.lines().enumerate() {
            let enables = line.contains("--features") || line.contains("features:");
            assert!(
                !(enables && line.contains(FEATURE)),
                "{name}:{} enables the test-only {FEATURE} feature: {}",
                number + 1,
                line.trim()
            );
        }
    }
    assert!(checked > 0, "no workflows found in {}", workflows.display());
    assert!(
        workflows.join(TEST_WORKFLOW).exists(),
        "{TEST_WORKFLOW} is missing"
    );
}
