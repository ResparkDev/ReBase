use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use ron::value::Value as RonValue;
use walkdir::WalkDir;

use crate::model::{CppField, CppSpec, DataFile, DataSpec, EnumDef, FieldSpec, RecordSpec, Ty};
use crate::options::Options;
use crate::{rustgen, ue};

/// Orchestrate scanning, parsing, validation, and backend generation.
///
/// - Scans `opts.input` for `.ron` files recursively
/// - Parses each file as either DataFile (multiple types) or DataSpec (single type)
/// - Validates and builds normalized specs
/// - Invokes backends:
///   - UE C++: writes headers/sources into `opts.out_ue`
///   - Rust single-file: writes `rebase_generated.rs` into `opts.out_rust_file`
///
/// Returns a list of generated unit descriptions (paths or identifiers).
pub fn run(opts: &Options) -> Result<Vec<String>> {
    opts.validate()?;

    if let Some(out) = &opts.out_ue {
        if !opts.dry_run {
            fs::create_dir_all(out)
                .with_context(|| format!("Failed to create UE out dir {}", out.display()))?;
        }
    }

    if let Some(out_file) = &opts.out_rust_file {
        if !opts.dry_run {
            if let Some(parent) = out_file.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create Rust out dir {}", parent.display())
                })?;
            }
        }
    }

    let ron_files = collect_ron_files(&opts.input)?;
    if ron_files.is_empty() {
        bail!(
            "No .ron files found under input directory: {}",
            opts.input.display()
        );
    }

    let mut generated_units = Vec::new();
    let mut all_specs: Vec<CppSpec> = Vec::new();

    for ron_path in ron_files {
        let content = fs::read_to_string(&ron_path)
            .with_context(|| format!("Failed to read {}", ron_path.display()))?;
        let specs = parse_specs(&content)
            .with_context(|| format!("Failed to parse RON file {}", ron_path.display()))?;

        for s in specs {
            let cpp_spec = build_cpp_spec(&s, opts.namespace.as_deref())?;

            if opts.dry_run {
                generated_units.push(format!(
                    "[dry-run] {}: {} record(s) from {}",
                    cpp_spec.type_name,
                    cpp_spec.records.len(),
                    ron_path.display()
                ));
            } else {
                if let Some(out_ue) = &opts.out_ue {
                    let unit = ue::generate_cpp_files(&cpp_spec, out_ue, true)?;
                    generated_units.push(format!("UE: {}", unit));
                }
                // Collect for Rust single-file emission at the end
                all_specs.push(cpp_spec);
            }
        }
    }

    if !opts.dry_run {
        if let Some(out_file) = &opts.out_rust_file {
            let unit = rustgen::generate_rust_single_file(&all_specs, out_file, true)?;
            generated_units.push(format!("Rust: {}", unit));
        }
    }

    Ok(generated_units)
}

// --------- Scanning and parsing ---------

fn collect_ron_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).follow_links(true) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if entry.path().extension() == Some(OsStr::new("ron")) {
                files.push(entry.into_path());
            }
        }
    }
    Ok(files)
}

fn parse_specs(content: &str) -> Result<Vec<DataSpec>> {
    // Try multi-type first
    if let Ok(df) = ron::de::from_str::<DataFile>(content) {
        return Ok(df.types);
    }
    // Try single type
    if let Ok(spec) = ron::de::from_str::<DataSpec>(content) {
        return Ok(vec![spec]);
    }
    Err(anyhow!("Content is neither DataFile nor DataSpec"))
}

// --------- Spec building and validation ---------

fn build_cpp_spec(spec: &DataSpec, cli_namespace: Option<&str>) -> Result<CppSpec> {
    // Merge namespace: CLI -> Spec with de-duplication if both are identical.
    let cli_chain: Vec<String> = cli_namespace
        .map(|ns| split_namespace(ns))
        .unwrap_or_default();
    let spec_chain: Vec<String> = spec
        .namespace
        .as_deref()
        .map(|ns| split_namespace(ns))
        .unwrap_or_default();

    let namespace_chain: Vec<String> = if !cli_chain.is_empty() && !spec_chain.is_empty() {
        if cli_chain == spec_chain {
            cli_chain.clone()
        } else {
            let mut merged = cli_chain.clone();
            merged.extend(spec_chain.iter().cloned());
            merged
        }
    } else if !cli_chain.is_empty() {
        cli_chain.clone()
    } else {
        spec_chain.clone()
    };

    // Fields mapping
    let mut cpp_fields = Vec::with_capacity(spec.fields.len());
    let mut enum_fields: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for f in &spec.fields {
        let ty = parse_ty(&f.ty).with_context(|| format!("Field '{}': invalid type", f.name))?;
        let cpp_type = match &ty {
            Ty::String => "FString".to_string(),
            Ty::FName => "FName".to_string(),
            Ty::Float => "float".to_string(),
            Ty::Double => "double".to_string(),
            Ty::Int32 => "int32".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Vector => "FVector".to_string(),
            Ty::Rotator => "FRotator".to_string(),
            Ty::Enum { name } => format!("E{}", sanitize_type_name(name)),
        };
        if let Ty::Enum { name } = &ty {
            enum_fields.entry(name.clone()).or_default();
        }
        cpp_fields.push(CppField {
            name: f.name.clone(),
            cpp_type,
            ty,
            doc: f.doc.clone(),
        });
    }

    // Validate records and collect enum values
    for r in &spec.records {
        // Ensure all fields present
        for f in &cpp_fields {
            if !r.values.contains_key(&f.name) {
                bail!("Record '{}' missing value for field '{}'", r.name, f.name);
            }
            let v = r.values.get(&f.name).unwrap();
            // Validate type compatibility shallowly and collect enum vals
            match &f.ty {
                Ty::Enum { name } => {
                    let s = expect_string(v).with_context(|| {
                        format!(
                            "Record '{}' field '{}' expected string for Enum({})",
                            r.name, f.name, name
                        )
                    })?;
                    let set = enum_fields
                        .get_mut(name)
                        .expect("enum set exists after field scan");
                    set.insert(s.to_string());
                }
                Ty::String | Ty::FName => {
                    expect_string(v).with_context(|| {
                        format!("Record '{}' field '{}' expected string", r.name, f.name)
                    })?;
                }
                Ty::Float => {
                    expect_number(v).with_context(|| {
                        format!("Record '{}' field '{}' expected float", r.name, f.name)
                    })?;
                }
                Ty::Double => {
                    expect_number(v).with_context(|| {
                        format!("Record '{}' field '{}' expected number", r.name, f.name)
                    })?;
                }
                Ty::Int32 => {
                    expect_integer(v).with_context(|| {
                        format!("Record '{}' field '{}' expected integer", r.name, f.name)
                    })?;
                }
                Ty::Bool => {
                    expect_bool(v).with_context(|| {
                        format!("Record '{}' field '{}' expected bool", r.name, f.name)
                    })?;
                }
                Ty::Vector => {
                    expect_vector(v).with_context(|| {
                        format!(
                            "Record '{}' field '{}' expected Vector (tuple [x,y,z] or map {{x,y,z}})",
                            r.name, f.name
                        )
                    })?;
                }
                Ty::Rotator => {
                    expect_rotator(v).with_context(|| {
                        format!(
                            "Record '{}' field '{}' expected Rotator (tuple [pitch,yaw,roll] or map {{pitch,yaw,roll}})",
                            r.name, f.name
                        )
                    })?;
                }
            }
        }
    }

    // Finalize enums
    let mut enums = Vec::new();
    for (ename, set) in enum_fields {
        let values: Vec<String> = set.into_iter().collect();
        if values.is_empty() {
            bail!("Enum({}) has no values collected from records", ename);
        }
        enums.push(EnumDef {
            name: ename,
            values,
        });
    }

    Ok(CppSpec {
        type_name: spec.type_name.clone(),
        namespace_chain,
        fields: cpp_fields,
        records: spec.records.clone(),
        enums,
    })
}

// --------- Shared helpers (used by backends too) ---------

pub(crate) fn split_namespace(ns: &str) -> Vec<String> {
    ns.split("::")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

pub(crate) fn sanitize_type_name(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if i == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "Type".to_string()
    } else {
        out
    }
}

pub(crate) fn sanitize_field_name(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if i == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "Field".to_string()
    } else {
        out
    }
}

pub(crate) fn enum_case(s: &str) -> String {
    // Convert to PascalCase-friendly enumerator
    let mut out = String::new();
    let mut upper_next = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper_next {
                for up in ch.to_uppercase() {
                    out.push(up);
                }
                upper_next = false;
            } else {
                out.push(ch);
            }
        } else {
            upper_next = true;
        }
    }
    if out.is_empty() {
        "Value".to_string()
    } else if out.chars().next().unwrap().is_ascii_digit() {
        format!("V{}", out)
    } else {
        out
    }
}

pub(crate) fn parse_ty(s: &str) -> Result<Ty> {
    let t = s.trim();
    if let Some(name) = parse_enum_ty(t) {
        return Ok(Ty::Enum { name });
    }
    match t {
        "String" => Ok(Ty::String),
        "FName" | "Name" => Ok(Ty::FName),
        "Float" | "f32" => Ok(Ty::Float),
        "Double" | "f64" => Ok(Ty::Double),
        "Int" | "i32" => Ok(Ty::Int32),
        "Bool" | "bool" => Ok(Ty::Bool),
        "Vector" => Ok(Ty::Vector),
        "Rotator" => Ok(Ty::Rotator),
        _ => bail!("Unsupported field type '{}'", s),
    }
}

pub(crate) fn parse_enum_ty(s: &str) -> Option<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^Enum\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*\)$").unwrap());
    re.captures(s).map(|cap| cap[1].to_string())
}

pub(crate) fn expect_string(v: &RonValue) -> Result<&str> {
    match v {
        RonValue::String(s) => Ok(s.as_str()),
        _ => bail!("expected string, got {:?}", v),
    }
}

pub(crate) fn expect_number(v: &RonValue) -> Result<f64> {
    match v {
        RonValue::Number(n) => Ok(n.clone().into_f64()),
        _ => bail!("expected number, got {:?}", v),
    }
}

pub(crate) fn expect_integer(v: &RonValue) -> Result<i64> {
    match v {
        RonValue::Number(n) => {
            if let Some(i) = n.clone().as_i64() {
                Ok(i)
            } else {
                bail!("expected integer, got float {:?}", n)
            }
        }
        _ => bail!("expected integer, got {:?}", v),
    }
}

pub(crate) fn expect_bool(v: &RonValue) -> Result<bool> {
    match v {
        RonValue::Bool(b) => Ok(*b),
        _ => bail!("expected bool, got {:?}", v),
    }
}

pub(crate) fn expect_vector(v: &RonValue) -> Result<(f32, f32, f32)> {
    match v {
        RonValue::Seq(seq) if seq.len() == 3 => {
            let x = expect_number(&seq[0])? as f32;
            let y = expect_number(&seq[1])? as f32;
            let z = expect_number(&seq[2])? as f32;
            Ok((x, y, z))
        }
        RonValue::Map(map) => {
            let mut as_map: HashMap<String, RonValue> = HashMap::new();
            for (k, val) in map.clone().into_iter() {
                if let RonValue::String(key) = k {
                    as_map.insert(key, val);
                }
            }
            let x = as_map
                .get("x")
                .or_else(|| as_map.get("X"))
                .ok_or_else(|| anyhow!("missing 'x'"))?;
            let y = as_map
                .get("y")
                .or_else(|| as_map.get("Y"))
                .ok_or_else(|| anyhow!("missing 'y'"))?;
            let z = as_map
                .get("z")
                .or_else(|| as_map.get("Z"))
                .ok_or_else(|| anyhow!("missing 'z'"))?;
            Ok((
                expect_number(x)? as f32,
                expect_number(y)? as f32,
                expect_number(z)? as f32,
            ))
        }
        _ => bail!("expected [x,y,z] or {{x,y,z}}"),
    }
}

pub(crate) fn expect_rotator(v: &RonValue) -> Result<(f32, f32, f32)> {
    match v {
        RonValue::Seq(seq) if seq.len() == 3 => {
            let p = expect_number(&seq[0])? as f32;
            let y = expect_number(&seq[1])? as f32;
            let r = expect_number(&seq[2])? as f32;
            Ok((p, y, r))
        }
        RonValue::Map(map) => {
            let mut as_map: HashMap<String, RonValue> = HashMap::new();
            for (k, val) in map.clone().into_iter() {
                if let RonValue::String(key) = k {
                    as_map.insert(key, val);
                }
            }
            let p = as_map
                .get("pitch")
                .or_else(|| as_map.get("Pitch"))
                .ok_or_else(|| anyhow!("missing 'pitch'"))?;
            let y = as_map
                .get("yaw")
                .or_else(|| as_map.get("Yaw"))
                .ok_or_else(|| anyhow!("missing 'yaw'"))?;
            let r = as_map
                .get("roll")
                .or_else(|| as_map.get("Roll"))
                .ok_or_else(|| anyhow!("missing 'roll'"))?;
            Ok((
                expect_number(p)? as f32,
                expect_number(y)? as f32,
                expect_number(r)? as f32,
            ))
        }
        _ => bail!("expected [pitch,yaw,roll] or {{pitch,yaw,roll}}"),
    }
}

pub(crate) fn quote_text_literal(s: &str) -> String {
    // Produce a C++ TEXT("...")-safe literal content; caller wraps with TEXT(...)
    let mut out = String::new();
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
