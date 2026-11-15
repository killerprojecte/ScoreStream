use crate::{CatalogError, Result};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
}

/// Values must come from the caller's approved target machine. This pure helper
/// reads no environment variables, registry, filesystem or account information.
#[derive(Default)]
pub struct RootContext<'a> {
    pub home: Option<&'a str>,
    pub local_app_data: Option<&'a str>,
    pub common_program_files: Option<&'a str>,
    pub common_program_files_x86: Option<&'a str>,
    pub application: Option<&'a str>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestedRoot {
    pub path: String,
    pub scope: String,
    pub priority: usize,
}

/// SDK discovery order. Suggestions are not evidence that a directory exists,
/// and do not authorize opening it. Use native Windows Known Folder values.
pub fn suggested_roots(platform: Platform, context: RootContext<'_>) -> Result<Vec<SuggestedRoot>> {
    let mut roots = Vec::new();
    let mut push = |base: &str, suffix: &str, scope: &str| -> Result<()> {
        let absolute = if platform == Platform::Windows {
            (base.len() >= 3
                && base.as_bytes()[0].is_ascii_alphabetic()
                && base.as_bytes()[1] == b':'
                && matches!(base.as_bytes()[2], b'/' | b'\\'))
                || base.starts_with("\\\\")
        } else {
            base.starts_with('/')
        };
        if !absolute || base.contains('\0') || base.split(['/', '\\']).any(|p| p == "..") {
            return Err(CatalogError::Invalid(
                "Root context must contain absolute, non-traversing paths".into(),
            ));
        }
        let separator = if platform == Platform::Windows {
            '\\'
        } else {
            '/'
        };
        let suffix = suffix.replace('/', &separator.to_string());
        let path = format!("{}{separator}{suffix}", base.trim_end_matches(['/', '\\']));
        if !roots.iter().any(|root: &SuggestedRoot| root.path == path) {
            roots.push(SuggestedRoot {
                path,
                scope: scope.into(),
                priority: roots.len(),
            });
        }
        Ok(())
    };
    match platform {
        Platform::Linux => {
            if let Some(home) = context.home {
                push(home, ".vst3", "user")?;
            }
            for base in [
                "/usr/lib64",
                "/usr/lib",
                "/usr/local/lib64",
                "/usr/local/lib",
            ] {
                push(base, "vst3", "global")?;
            }
            if let Some(app) = context.application {
                push(app, "vst3", "application")?;
            }
        }
        Platform::MacOs => {
            if let Some(home) = context.home {
                push(home, "Library/Audio/Plug-ins/VST3", "user")?;
            }
            push("/Library", "Audio/Plug-ins/VST3", "global")?;
            push("/Network/Library", "Audio/Plug-ins/VST3", "global")?;
            if let Some(app) = context.application {
                push(app, "Contents/VST3", "application")?;
            }
        }
        Platform::Windows => {
            if let Some(user) = context.local_app_data {
                push(user, "Programs/Common/VST3", "user")?;
            }
            if let Some(global) = context.common_program_files {
                push(global, "VST3", "global")?;
            }
            if let Some(global) = context.common_program_files_x86 {
                push(global, "VST3", "global_x86")?;
            }
            if let Some(app) = context.application {
                push(app, "VST3", "application")?;
            }
        }
    }
    Ok(roots)
}
