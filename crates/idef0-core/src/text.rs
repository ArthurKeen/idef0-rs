//! Names, keys and descriptive text (`docs/spec/metamodel.md` §3.3, §4.3, §12; FR-ID-03,
//! FR-ID-05, FR-NAME-01, FR-NAME-12).

use core::fmt;
use core::str::FromStr;

use compact_str::CompactString;
use serde::{Deserialize, Serialize};

/// A name that is not empty after trimming. Box and concept names have this type, so a box
/// or concept without a name cannot be constructed (SR-37).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonEmptyString(CompactString);

/// A name was empty or only whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a name must not be empty")]
pub struct EmptyName;

impl NonEmptyString {
    /// The trimmed text, or [`EmptyName`] if nothing is left.
    ///
    /// # Errors
    ///
    /// [`EmptyName`] if `s` is empty or only whitespace.
    pub fn new(s: impl AsRef<str>) -> Result<Self, EmptyName> {
        let t = s.as_ref().trim();
        if t.is_empty() {
            Err(EmptyName)
        } else {
            Ok(Self(CompactString::new(t)))
        }
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for NonEmptyString {
    type Err = EmptyName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A key was malformed (`E-ID-05`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "invalid key {0:?}: expected a lowercase letter or digit, then up to 63 lowercase letters, digits or `-` (E-ID-05)"
)]
pub struct KeyError(pub String);

/// `Key ::= [a-z0-9] [a-z0-9-]{0,63}` (metamodel §3.6).
fn is_key(s: &str) -> bool {
    let b = s.as_bytes();
    let word = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    (1..=64).contains(&b.len()) && word(&b[0]) && b[1..].iter().all(|c| word(c) || *c == b'-')
}

macro_rules! key_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(CompactString);

        impl $name {
            /// The key as written.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// A key from a string the slug rule produced, which is always valid.
            #[allow(dead_code, reason = "arrow keys are written by hand, never minted")]
            pub(crate) fn from_slug(s: &str) -> Self {
                debug_assert!(is_key(s), "slug {s:?} is not a key");
                Self(CompactString::new(s))
            }
        }

        impl FromStr for $name {
            type Err = KeyError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if is_key(s) {
                    Ok(Self(CompactString::new(s)))
                } else {
                    Err(KeyError(s.to_owned()))
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        serde_via_str!($name);
    };
}

key_type!(
    /// A box key (FR-ID-03): a stable slug, unique per model, that never changes on rename,
    /// move or renumber (FR-ID-04).
    BoxKey
);
key_type!(
    /// A concept key (FR-ID-05), in a namespace separate from box keys.
    ConceptKey
);
key_type!(
    /// An explicit arrow key (`key=` on an arrow, metamodel §3.4), preserved verbatim.
    ArrowKey
);

serde_via_str!(NonEmptyString);

/// Where a name or description came from (FR-NAME-12). A human edit resets it to `Model`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Written in the model.
    #[default]
    Model,
    /// Imported from a PRD.
    Prd,
    /// Derived from code.
    Code,
    /// Proposed by an LLM and accepted (D15).
    Llm,
}

impl Origin {
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference
    fn is_model(&self) -> bool {
        *self == Self::Model
    }
}

/// Provenance of a name or description (FR-NAME-12): `origin=` and `source=` in the DSL.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Provenance {
    /// Where the text came from.
    #[serde(default, skip_serializing_if = "Origin::is_model")]
    pub origin: Origin,
    /// A reference to the source, such as a PRD anchor or a code path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Provenance {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A Markdown description with its provenance (FR-NAME-01, FR-NAME-07, FR-NAME-12).
///
/// In JSON a description with default provenance is a plain string; otherwise it is an
/// object with `text`, `origin` and `source`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "DescriptionRepr", into = "DescriptionRepr")]
pub struct Description {
    /// The Markdown text; empty when the element is undescribed.
    pub text: String,
    /// Where the text came from.
    pub provenance: Provenance,
}

impl Description {
    /// A description written in the model.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            provenance: Provenance::default(),
        }
    }

    /// Whether there is no text and no provenance.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.provenance.is_default()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum DescriptionRepr {
    Text(String),
    Full {
        text: String,
        #[serde(flatten)]
        provenance: Provenance,
    },
}

impl From<DescriptionRepr> for Description {
    fn from(r: DescriptionRepr) -> Self {
        match r {
            DescriptionRepr::Text(text) => Self::new(text),
            DescriptionRepr::Full { text, provenance } => Self { text, provenance },
        }
    }
}

impl From<Description> for DescriptionRepr {
    fn from(d: Description) -> Self {
        if d.provenance.is_default() {
            Self::Text(d.text)
        } else {
            Self::Full {
                text: d.text,
                provenance: d.provenance,
            }
        }
    }
}

/// A C-number (§2.11, Annex C.4.5.1): 2–3 author initials and a sequence, such as `AK14`,
/// optionally with the number it replaces, `AK15(AK14)`. Spaces in the input are removed
/// (`AK 14` → `AK14`, metamodel §4.3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CNumber(CompactString);

/// A C-number was malformed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "invalid C-number {0:?}: expected 2-3 capital initials and a sequence, e.g. AK14 or AK15(AK14)"
)]
pub struct CNumberError(pub String);

impl CNumber {
    /// The normalized C-number.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_cnum(s: &str) -> bool {
    let letters = s.bytes().take_while(u8::is_ascii_uppercase).count();
    let digits = &s[letters..];
    (2..=3).contains(&letters) && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

impl FromStr for CNumber {
    type Err = CNumberError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let compact: String = s.chars().filter(|c| *c != ' ').collect();
        let valid = match compact.split_once('(') {
            None => is_cnum(&compact),
            Some((current, rest)) => rest
                .strip_suffix(')')
                .is_some_and(|previous| is_cnum(current) && is_cnum(previous)),
        };
        if valid {
            Ok(Self(CompactString::new(compact)))
        } else {
            Err(CNumberError(s.to_owned()))
        }
    }
}

impl fmt::Display for CNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

serde_via_str!(CNumber);

/// A BCP 47 language tag (FR-META-05), default `en`. Checked for shape only: subtags of 1–8
/// ASCII letters or digits separated by `-`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LangTag(CompactString);

/// A language tag was malformed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid language tag {0:?}: expected BCP 47, e.g. en or de-CH")]
pub struct LangTagError(pub String);

impl LangTag {
    /// The tag as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn is_en(&self) -> bool {
        self.0 == "en"
    }
}

impl Default for LangTag {
    fn default() -> Self {
        Self(CompactString::const_new("en"))
    }
}

impl FromStr for LangTag {
    type Err = LangTagError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let valid = s.split('-').all(|sub| {
            (1..=8).contains(&sub.len()) && sub.bytes().all(|b| b.is_ascii_alphanumeric())
        });
        if valid {
            Ok(Self(CompactString::new(s)))
        } else {
            Err(LangTagError(s.to_owned()))
        }
    }
}

impl fmt::Display for LangTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

serde_via_str!(LangTag);

pub(crate) fn lang_is_en(tag: &LangTag) -> bool {
    tag.is_en()
}

#[cfg(test)]
mod tests {
    use super::*;

    // FR-NAME-01, SR-37: names are trimmed and never empty.
    #[test]
    fn non_empty_string() {
        assert_eq!(
            NonEmptyString::new("  Validate Order ").unwrap().as_str(),
            "Validate Order"
        );
        assert_eq!(NonEmptyString::new(""), Err(EmptyName));
        assert_eq!(NonEmptyString::new(" \t\n"), Err(EmptyName));
    }

    // testing.md §11: deserializing "" is a typed error.
    #[test]
    fn empty_name_does_not_deserialize() {
        let err = serde_json::from_str::<NonEmptyString>("\"\"").unwrap_err();
        assert!(
            err.to_string().contains("a name must not be empty"),
            "{err}"
        );
        let ok: NonEmptyString = serde_json::from_str("\" Plan \"").unwrap();
        assert_eq!(ok.as_str(), "Plan");
    }

    // FR-ID-03, FR-ID-05, E-ID-05: Key ::= [a-z0-9] [a-z0-9-]{0,63}.
    #[test]
    fn keys() {
        for ok in ["a", "0", "validate-order", "a-", "x2-3"] {
            assert_eq!(ok.parse::<BoxKey>().unwrap().as_str(), ok);
            assert!(ok.parse::<ConceptKey>().is_ok());
        }
        let long = "a".repeat(64);
        assert!(long.parse::<ArrowKey>().is_ok());
        let too_long = "a".repeat(65);
        for bad in ["", "-a", "Validate", "a_b", "a b", "é", too_long.as_str()] {
            assert_eq!(
                bad.parse::<BoxKey>(),
                Err(KeyError(bad.to_owned())),
                "{bad:?}"
            );
        }
    }

    // metamodel §4.3, Annex C.4.5.1.
    #[test]
    fn cnumbers() {
        assert_eq!("AK14".parse::<CNumber>().unwrap().as_str(), "AK14");
        assert_eq!("AK 14".parse::<CNumber>().unwrap().as_str(), "AK14");
        assert_eq!("ABC7".parse::<CNumber>().unwrap().as_str(), "ABC7");
        assert_eq!(
            "AK15(AK 14)".parse::<CNumber>().unwrap().as_str(),
            "AK15(AK14)"
        );
        for bad in [
            "A14", "ABCD1", "AK", "ak14", "AK14(", "AK14()", "AK14(A1)", "14AK",
        ] {
            assert!(bad.parse::<CNumber>().is_err(), "{bad:?}");
        }
    }

    // FR-META-05.
    #[test]
    fn lang_tags() {
        assert_eq!(LangTag::default().as_str(), "en");
        for ok in ["en", "de-CH", "zh-Hant-TW"] {
            assert!(ok.parse::<LangTag>().is_ok(), "{ok:?}");
        }
        for bad in ["", "en-", "-en", "toolongtag", "en_US"] {
            assert!(bad.parse::<LangTag>().is_err(), "{bad:?}");
        }
    }

    // FR-NAME-12: default provenance is a plain string in JSON; otherwise an object.
    #[test]
    fn description_json() {
        let plain = Description::new("Checks the order.");
        assert_eq!(
            serde_json::to_string(&plain).unwrap(),
            r#""Checks the order.""#
        );
        let sourced = Description {
            text: "From the PRD.".into(),
            provenance: Provenance {
                origin: Origin::Prd,
                source: Some("PRD.md#orders".into()),
            },
        };
        let json = serde_json::to_string(&sourced).unwrap();
        assert_eq!(
            json,
            r#"{"text":"From the PRD.","origin":"prd","source":"PRD.md#orders"}"#
        );
        assert_eq!(serde_json::from_str::<Description>(&json).unwrap(), sourced);
        assert_eq!(
            serde_json::from_str::<Description>(r#""x""#).unwrap(),
            Description::new("x")
        );
    }
}
