//! Live installed-library check, not a source/bank/render authorization receipt.
use crate::{BackendCapabilities, Result, native};
#[derive(Debug, serde::Serialize)]
pub struct DryBackendProbe {
    pub capabilities: BackendCapabilities,
    pub physical_group_limit: usize,
    pub bank_verified: bool,
    pub source_plan_verified: bool,
}
/// Uses the unchanged fixed-path/ownership/ELF/hash/ABI loader and exact dry symbol
/// table. Opens no soundfont, audio device or MIDI driver and performs no rendering.
/// Every actual export still repeats its normal native/source/bank verification.
pub fn installed() -> Result<DryBackendProbe> {
    let api = native::Api::open()?;
    let physical_group_limit = native::dry::probe_api(&api)?;
    Ok(DryBackendProbe { capabilities:api.capabilities.clone(), physical_group_limit,
        bank_verified:false, source_plan_verified:false })
}
#[cfg(test)]
mod tests {
    #[test]
    fn loader_override_rejection_is_not_reported_available() {
        const FLAG: &str = "SCORESTREAM_PROBE_CHILD";
        if std::env::var_os(FLAG).is_some() {
            assert!(matches!(super::installed(), Err(crate::AudioError::Unavailable(_))));
            return;
        }
        // A child-only unknown LD_ marker exercises the existing fail-closed
        // loader guard. It does not alter system settings or the parent's loader.
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(FLAG,"1").env("LD_SCORESTREAM_PROBE_TEST","refuse")
            .args(["--exact","backend_probe::tests::loader_override_rejection_is_not_reported_available"])
            .status().unwrap();
        assert!(status.success());
    }
    #[test]
    #[ignore="requires trusted installed FluidSynth ABI; run binary without Cargo loader overrides"]
    fn actual_installed_dry_abi_is_verified_without_bank() {
        let report=super::installed().unwrap();
        assert!(report.capabilities.version.starts_with("2."));
        assert_eq!(report.capabilities.checked_library_sha256.len(),64);
        assert!(report.physical_group_limit>0);
        assert!(!report.bank_verified && !report.source_plan_verified);
        println!("{}",serde_json::to_string(&report).unwrap());
    }
}
