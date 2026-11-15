use crate::{CatalogError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Canonical 32-hex-character moduleinfo CID, not a host-endian COM GUID.
/// Stable vendor identity claim; it must be checked against a future live factory.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ClassId(String);
impl ClassId {
    pub fn parse(value: &str) -> Result<Self> {
        if value.len() != 32 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CatalogError::Invalid(
                "CID must be exactly 32 hexadecimal characters".into(),
            ));
        }
        Ok(Self(value.to_ascii_uppercase()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn logical_key(&self) -> String {
        format!("vst3:{}", self.0)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ClassInfo {
    pub cid: ClassId,
    pub name: String,
    pub category: String,
    pub vendor: String,
    pub version: String,
    pub sdk_version: String,
    pub sub_categories: Vec<String>,
    pub class_flags: Option<u32>,
    pub cardinality: Option<i32>,
}
impl ClassInfo {
    pub fn claims_instrument(&self) -> bool {
        self.category == "Audio Module Class"
            && self.sub_categories.iter().any(|item| item == "Instrument")
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Compatibility {
    pub new: ClassId,
    pub old: Vec<ClassId>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    pub factory_vendor: String,
    pub classes: Vec<ClassInfo>,
    pub compatibility_claims: Vec<Compatibility>,
}

#[derive(Deserialize)]
struct RawModule {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version", default)]
    version: String,
    #[serde(rename = "Factory Info", default)]
    factory: RawFactory,
    #[serde(rename = "Classes")]
    classes: Vec<RawClass>,
    #[serde(rename = "Compatibility", default)]
    compatibility: Vec<RawCompatibility>,
}
#[derive(Default, Deserialize)]
struct RawFactory {
    #[serde(rename = "Vendor", default)]
    vendor: String,
}
#[derive(Deserialize)]
struct RawClass {
    #[serde(rename = "CID")]
    cid: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Category")]
    category: String,
    #[serde(rename = "Vendor", default)]
    vendor: String,
    #[serde(rename = "Version", default)]
    version: String,
    #[serde(rename = "SDKVersion", default)]
    sdk_version: String,
    #[serde(rename = "Sub Categories", default)]
    sub_categories: Vec<String>,
    #[serde(rename = "Class Flags")]
    class_flags: Option<f64>,
    #[serde(rename = "Cardinality")]
    cardinality: Option<f64>,
}
#[derive(Deserialize)]
struct RawCompatibility {
    #[serde(rename = "New")]
    new: String,
    #[serde(rename = "Old")]
    old: Vec<String>,
}

fn short(value: &str) -> Result<()> {
    if value.len() > 4096 || value.contains('\0') {
        return Err(CatalogError::Limit(
            "Metadata string exceeds 4096 bytes or contains NUL".into(),
        ));
    }
    Ok(())
}

/// Full JSON5 grammar is delegated to the pinned established parser. The small
/// lexical pass below only caps nesting before the parser allocates/recurses.
pub fn parse_moduleinfo(text: &str) -> Result<ModuleInfo> {
    if text.len() > 1024 * 1024 {
        return Err(CatalogError::Limit("Manifest exceeds 1 MiB".into()));
    }
    guard_depth(text)?;
    let raw: RawModule = json5::from_str(text)
        .map_err(|error| CatalogError::Invalid(format!("moduleinfo JSON5: {error}")))?;
    if raw.classes.len() > 2048 || raw.compatibility.len() > 2048 {
        return Err(CatalogError::Limit(
            "Too many declared classes/compatibility entries".into(),
        ));
    }
    for value in [&raw.name, &raw.version, &raw.factory.vendor] {
        short(value)?;
    }
    let mut ids = BTreeSet::new();
    let mut classes = Vec::new();
    for item in raw.classes {
        let cid = ClassId::parse(&item.cid)?;
        if !ids.insert(cid.clone()) {
            return Err(CatalogError::Invalid("Duplicate CID in module".into()));
        }
        for value in [
            &item.name,
            &item.category,
            &item.vendor,
            &item.version,
            &item.sdk_version,
        ] {
            short(value)?;
        }
        if item.sub_categories.len() > 64 {
            return Err(CatalogError::Limit("Too many subcategories".into()));
        }
        for value in &item.sub_categories {
            short(value)?;
        }
        let class_flags = item
            .class_flags
            .map(|value| {
                if !value.is_finite()
                    || value.fract() != 0.0
                    || !(0.0..=f64::from(u32::MAX)).contains(&value)
                {
                    return Err(CatalogError::Invalid(
                        "Class Flags must be a finite u32 integer".into(),
                    ));
                }
                Ok(value as u32)
            })
            .transpose()?;
        let cardinality = item
            .cardinality
            .map(|value| {
                if !value.is_finite()
                    || value.fract() != 0.0
                    || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value)
                {
                    return Err(CatalogError::Invalid(
                        "Cardinality must be a finite i32 integer".into(),
                    ));
                }
                Ok(value as i32)
            })
            .transpose()?;
        classes.push(ClassInfo {
            cid,
            name: item.name,
            category: item.category,
            vendor: item.vendor,
            version: item.version,
            sdk_version: item.sdk_version,
            sub_categories: item.sub_categories,
            class_flags,
            cardinality,
        });
    }
    let mut compatibility_claims = Vec::new();
    for entry in raw.compatibility {
        let new = ClassId::parse(&entry.new)?;
        if !ids.contains(&new) {
            return Err(CatalogError::Invalid(
                "Compatibility target is not declared by this module".into(),
            ));
        }
        if entry.old.len() > 2048 {
            return Err(CatalogError::Limit("Too many compatibility IDs".into()));
        }
        let old = entry
            .old
            .iter()
            .map(|id| ClassId::parse(id))
            .collect::<Result<_>>()?;
        compatibility_claims.push(Compatibility { new, old });
    }
    Ok(ModuleInfo {
        name: raw.name,
        version: raw.version,
        factory_vendor: raw.factory.vendor,
        classes,
        compatibility_claims,
    })
}

fn guard_depth(text: &str) -> Result<()> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut depth = 0_usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                let quote = bytes[index];
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index += 2;
                        continue;
                    }
                    if bytes[index] == quote {
                        index += 1;
                        break;
                    }
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                // JSON5 also recognizes Unicode line separators.
                while index < bytes.len()
                    && !matches!(bytes[index], b'\r' | b'\n')
                    && !text.as_bytes()[index..].starts_with(&[0xe2, 0x80, 0xa8])
                    && !text.as_bytes()[index..].starts_with(&[0xe2, 0x80, 0xa9])
                {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            b'{' | b'[' => {
                depth += 1;
                if depth > 32 {
                    return Err(CatalogError::Limit("Manifest nesting exceeds 32".into()));
                }
                index += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            _ => index += 1,
        }
    }
    Ok(())
}
