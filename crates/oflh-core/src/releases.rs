//! Stable-release comparison shared by frontends and transport adapters.
use semver::Version;
use thiserror::Error;

/// Invalid version data, retaining the original parser error when available.
#[derive(Debug, Error)]
pub enum ReleaseVersionError {
    /// Version strings are bounded before parsing or displaying remote data.
    #[error("{operation}: version exceeds 128 bytes")]
    TooLong {
        /// Which version could not be parsed.
        operation: &'static str,
    },
    /// A version did not follow full semantic-version syntax.
    #[error("{operation}: {source}")]
    Invalid {
        /// Which version could not be parsed.
        operation: &'static str,
        /// Original semantic-version parser failure.
        #[source]
        source: semver::Error,
    },
}

fn parse_version(value: &str, operation: &'static str) -> Result<Version, ReleaseVersionError> {
    if value.len() > 128 {
        return Err(ReleaseVersionError::TooLong { operation });
    }
    Version::parse(value.strip_prefix('v').unwrap_or(value))
        .map_err(|source| ReleaseVersionError::Invalid { operation, source })
}

/// Whether a build has a usable version for update checks.
/// Unversioned development and pull-request artifacts do not check automatically.
pub fn can_check_releases(version: &str, pull_request_url: &str) -> bool {
    pull_request_url.is_empty() && parse_version(version, "installed version").is_ok()
}

/// Return a newer stable release, ignoring build metadata for precedence.
/// Prereleases are never offered through the stable channel. A stable release
/// of the same version supersedes an installed release candidate.
pub fn newer_stable_release(
    installed: &str,
    candidate: &str,
) -> Result<Option<String>, ReleaseVersionError> {
    let installed = parse_version(installed, "installed version")?;
    let mut candidate = parse_version(candidate, "release version")?;
    if !candidate.pre.is_empty() || !candidate.cmp_precedence(&installed).is_gt() {
        return Ok(None);
    }
    candidate.build = semver::BuildMetadata::EMPTY;
    Ok(Some(candidate.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_channel_obeys_numeric_precedence_and_ignores_build_metadata() {
        for (installed, candidate, expected) in [
            ("0.7.0-rc.10", "v0.7.0", Some("0.7.0")),
            ("0.7.0", "0.7.0+new", None),
            ("0.7.0+old", "0.7.0+new", None),
            ("0.7.0-rc.1", "0.6.99", None),
            ("0.7.0", "0.8.0-rc.1", None),
            ("1.9.0", "1.10.0+build", Some("1.10.0")),
            ("2.0.0", "1.999.0", None),
        ] {
            assert_eq!(
                newer_stable_release(installed, candidate)
                    .unwrap()
                    .as_deref(),
                expected
            );
        }
    }
    #[test]
    fn invalid_or_unversioned_builds_do_not_check_and_errors_keep_context() {
        assert!(!can_check_releases("development", ""));
        assert!(!can_check_releases(
            "0.7.0",
            "https://example.invalid/pull/1"
        ));
        assert!(can_check_releases("0.7.0-rc.1+build", ""));
        for version in ["1.0", "01.0.0", "1.0.0\n", "vv1.0.0", "1.0.0-rc.01"] {
            assert!(matches!(
                newer_stable_release("1.0.0", version),
                Err(ReleaseVersionError::Invalid {
                    operation: "release version",
                    ..
                })
            ));
        }
        assert!(matches!(
            newer_stable_release("1.0.0", &"1".repeat(129)),
            Err(ReleaseVersionError::TooLong { .. })
        ));
    }
}
