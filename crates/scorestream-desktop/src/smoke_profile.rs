//! The native smoke feature is opt-in, and its profile must also be isolated.

pub fn ensure_isolated_identifier(identifier: &str) -> Result<(), &'static str> {
    if identifier == "io.scorestream.desktop.qa" {
        Ok(())
    } else {
        Err("Smoke runs require tauri.smoke.conf.json and the isolated io.scorestream.desktop.qa identifier; refusing to access the production profile")
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_isolated_identifier;

    #[test]
    fn permits_only_the_explicit_qa_profile() {
        assert!(ensure_isolated_identifier("io.scorestream.desktop.qa").is_ok());
        for identifier in ["io.scorestream.desktop", "", "io.scorestream.desktop.qa.other"] {
            assert!(ensure_isolated_identifier(identifier).is_err());
        }
    }
}
