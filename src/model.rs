#![allow(missing_docs)]

use ron::value::Value as RonValue;
use serde::Deserialize;
use std::collections::HashMap;

/// Root container for files that define multiple types.
/// RON shape:
/// (
///     types: [ ( ...DataSpec... ), ... ]
/// )
#[derive(Debug, Deserialize)]
pub(crate) struct DataFile {
    pub types: Vec<DataSpec>,
}

/// Single type definition within a RON file.
/// RON shape:
/// (
///     type_name: "Weapon",
///     namespace: Some("Game::DB"), // optional
///     fields: [ (name: "Id", ty: "FName"), ... ],
///     records: [ (name: "AssaultRifle1", values: { "Id": "...", ... }), ... ],
/// )
#[derive(Debug, Deserialize)]
pub(crate) struct DataSpec {
    pub type_name: String,
    #[serde(default)]
    pub namespace: Option<String>,
    pub fields: Vec<FieldSpec>,
    #[serde(default)]
    pub records: Vec<RecordSpec>,
}

/// Field metadata as authored in RON.
#[derive(Debug, Deserialize)]
pub(crate) struct FieldSpec {
    pub name: String,
    pub ty: String,
    #[serde(default)]
    pub doc: Option<String>,
}

/// A single record (row) instance as authored in RON.
/// `values` is a string-keyed map (e.g., { "Id": "AK", "Dps": 42.0, ... }).
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct RecordSpec {
    pub name: String,
    #[serde(default)]
    pub doc: Option<String>,
    #[serde(default)]
    pub values: HashMap<String, RonValue>,
}

/// Internal representation for generated enums.
/// Collected from fields declared as `Enum(Name)` by scanning record values.
#[derive(Debug, Clone)]
pub(crate) struct EnumDef {
    pub name: String,
    /// Ordered set collected at build-time (duplicates removed earlier).
    pub values: Vec<String>,
}

/// Internal normalized field used across both backends.
#[derive(Debug)]
pub(crate) struct CppField {
    pub name: String,
    pub cpp_type: String,
    pub ty: Ty,
    pub doc: Option<String>,
}

/// Internal normalized spec used by UE and Rust generation backends.
#[derive(Debug)]
pub(crate) struct CppSpec {
    pub type_name: String,
    /// Namespace chain (outermost first); combined from CLI and per-file `namespace`.
    pub namespace_chain: Vec<String>,
    pub fields: Vec<CppField>,
    pub records: Vec<RecordSpec>,
    pub enums: Vec<EnumDef>,
}

/// Normalized field type variants supported by both backends.
#[derive(Debug, Clone)]
pub(crate) enum Ty {
    String,
    FName,
    Float,
    Double,
    Int32,
    Bool,
    Vector,
    Rotator,
    Enum { name: String },
}
