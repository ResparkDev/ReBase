# ReBase


ReBase is a RON → Unreal Engine 5 (and Rust) static data generator. It turns human-friendly RON schemas into:

- UE5 C++ headers/sources (plain C++, no UHT; fast and easy to use in engine code)

- A single Rust source file (rebase_generated.rs) for servers



This gives you one source of truth for gameplay data shared by client and server with zero runtime parsing and no runtime loaders.

--------------------------------------------------------------------------------

## Why ReBase?

- One schema, two targets: UE C++ and Rust
- Human-friendly authoring with RON
- Strong compile-time typing (structs and enums)
- Zero runtime parsing; data is compiled into your binaries
- Simple and fast build integration on both sides

--------------------------------------------------------------------------------

## Install

- Build from source:
  - Debug: `cargo build`
  - Release: `cargo build --release`
  - Install locally: `cargo install --path .`

- CLI help:
  - `rebase --help`

Requirements:
- Rust toolchain (stable)
- UE5 C++ project if generating UE
- A Rust server project if generating Rust

--------------------------------------------------------------------------------

## CLI

ReBase scans an input directory recursively for `.ron` files, validates them, and generates code to the selected outputs.

Common invocations:
- UE only:
  - `rebase --input ./Data --out-ue ./UE/Source/MyGame/ReBase --namespace Game::DB`
- Rust only (single file):
  - `rebase --input ./Data --out-rust-file ./target/rebase_generated.rs --namespace Game::DB`
- Both:
  - `rebase --input ./Data --out-ue ./UE/Source/MyGame/ReBase --out-rust-file ./target/rebase_generated.rs --namespace Game::DB`
- Validate only:
  - `rebase --input ./Data --dry-run`

Options:
- `-i, --input <DIR>`        Input directory containing `.ron` files (recursively)
- `--out-ue <DIR>`           Output directory for UE C++ headers/sources (optional)
- `--out-rust-file <FILE>`   Output file path for the single Rust module (optional)
- `--namespace <Chain>`      Optional C++ namespace chain (e.g., `Game::DB`)
- `--force`                  Ignored (outputs are always overwritten on each run)
- `--dry-run`                Parse/validate and print plan; do not write files

Notes:
- Outputs are overwritten on each run (no prompt). Use VCS to review diffs.
- The `--namespace` is used for UE namespaces and to nest modules in the Rust single-file output. If the CLI `--namespace` and a file `namespace:` are identical, they are deduplicated (not double-nested).

--------------------------------------------------------------------------------

## RON schema

You can define a single type per file (DataSpec) or multiple types per file (DataFile).

Single type (DataSpec):
```
(
    type_name: "Weapon",
    namespace: Some("Game::DB"),
    fields: [
        (name: "Id",           ty: "FName"),
        (name: "Dps",          ty: "Float"),
        (name: "Clips",        ty: "Int"),
        (name: "IsHitscan",    ty: "Bool"),
        (name: "Kind",         ty: "Enum(WeaponKind)"),
        (name: "MuzzleOffset", ty: "Vector"),
    ],
    records: [
        (name: "AssaultRifle1", values: {
            "Id": "AssaultRifle1",
            "Dps": 42.5,
            "Clips": 3,
            "IsHitscan": true,
            "Kind": "AssaultRifle",
            "MuzzleOffset": (0.0, 0.0, 20.0),
        }),
        (name: "Shotgun2", values: {
            "Id": "Shotgun2",
            "Dps": 12.75,
            "Clips": 8,
            "IsHitscan": false,
            "Kind": "Shotgun",
            "MuzzleOffset": { "x": 0.0, "y": 1.0, "z": 10.0 },
        }),
    ],
)
```

Multiple types (DataFile):
```
(
    types: [
        (
            type_name: "Weapon",
            fields: [ /* ... */ ],
            records: [ /* ... */ ],
        ),
        (
            type_name: "Ammo",
            fields: [ /* ... */ ],
            records: [ /* ... */ ],
        ),
    ]
)
```

Supported field types:
- `String`      → UE: `FString` | Rust: `&'static str`
- `FName`/`Name`→ UE: `FName`   | Rust: `&'static str`
- `Float`/`f32` → UE: `float`   | Rust: `f32`
- `Double`/`f64`→ UE: `double`  | Rust: `f64`
- `Int`/`i32`   → UE: `int32`   | Rust: `i32`
- `Bool`/`bool` → UE: `bool`    | Rust: `bool`
- `Vector`      → UE: `FVector` | Rust: `struct Vector { x, y, z }`
  - Value forms: `(x, y, z)` or `{ "x": f, "y": f, "z": f }`
- `Rotator`     → UE: `FRotator`| Rust: `struct Rotator { pitch, yaw, roll }`
  - Value forms: `(pitch, yaw, roll)` or `{ "pitch": f, "yaw": f, "roll": f }`
- `Enum(Name)`  → UE: `enum class EName : uint8` | Rust: `enum Name`

Validation:
- Every record must provide a value for every declared field.
- Types are validated:
  - Numeric types must be numbers; `Int` must be an integer.
  - `Bool` must be true/false.
  - `Vector`/`Rotator` must be proper tuples or maps.
  - For map-forms, quote inner keys: `"x","y","z","pitch","yaw","roll"`.
- Enum(Name) field values must be strings; their set of values is inferred from your records.

Encoding tip:
- Save `.ron` files as UTF-8 without BOM.

--------------------------------------------------------------------------------

## UE backend

What gets generated (per type):
- `<Type>ReBase.h`
- `<Type>ReBase.cpp`

API shape:
- `struct F<Type>Row { FString Name; ... fields ... }`
- `const TArray<F<Type>Row>& Get<Type>Data();`
- `const F<Type>Row* Find<Type>ByName(FStringView Name);`
- For `Enum(Name)` fields:
  - `enum class EName : uint8 { /* PascalCase enumerators */ };`

Usage (example):
```
#include "WeaponReBase.h"

namespace Game { namespace DB {

void UseWeapons()
{
    const auto& Data = GetWeaponData();
    for (const FWeaponRow& Row : Data)
    {
        UE_LOG(LogTemp, Log, TEXT("Weapon %s DPS=%f"), *Row.Name, Row.Dps);
    }

    if (const FWeaponRow* Shotgun = FindWeaponByName(TEXT("Shotgun2")))
    {
        // Shotgun->Kind, Shotgun->MuzzleOffset, etc.
    }
}

}} // namespace
```


Recommended output:

- `UE/Source/<YourModule>/ReBase` or any folder compiled by your module


UE5 integration (step-by-step):
1) Choose an output folder inside your game module
- Typical: `Source/<YourModule>/Private/Generated` for `.h/.cpp` used only within the module.
- If other modules need to include the generated headers, put headers under `Source/<YourModule>/Public/Generated` and keep `.cpp` under `Source/<YourModule>/Private/Generated`.

2) Run ReBase to generate UE files
- Example:
  - `rebase --input ./Data --out-ue ./UE/Source/MyGame/Private/Generated --namespace Game::DB`

3) Make sure the files are compiled by your module
- Any `.cpp` under `Source/<YourModule>/Private` (or its subfolders) is compiled automatically by Unreal Build Tool.

4) Include and use in C++
- Include the generated headers from your module sources (e.g., `#include "WeaponReBase.h"`) and call `Get<Type>Data()` / `Find<Type>ByName()` as shown above.

5) CI integration (optional)
- Generate during CI and publish artifacts into the client repo, or commit generated files to VCS if you prefer deterministic builds.
- After adding new generated `.cpp`/`.h`, regenerate/refresh your project files in your IDE if they are not picked up automatically.

--------------------------------------------------------------------------------


## Rust backend (single-file)

What gets generated:
- A single file you choose (e.g., `./target/rebase_generated.rs`) containing nested modules by namespace and per-type modules with:
  - `enum` declarations (for `Enum(Name)` fields)
  - `Vector`/`Rotator` helper structs as needed
  - `pub struct <Type>Row`
  - `pub static <TYPE>_DATA: &[<Type>Row]`
  - `pub fn get_<type>_data() -> &'static [<Type>Row]`
  - `pub fn find_<type>_by_name(name: &str) -> Option<&'static <Type>Row>`

Notes:
- Strings are `&'static str` in generated rows, so they can live in `static` data without allocation.
- The generated file is self-contained (no external crate).

Integrate with build.rs:
```
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let data_dir = manifest_dir.join("data");

    // Rebuild when any .ron changes
    println!("cargo:rerun-if-changed={}", data_dir.display());

    // Generate single Rust file into OUT_DIR
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rust_out_file = out_dir.join("rebase_generated.rs");

    // Optionally, also generate UE C++ for a sibling client project
    let ue_cpp_out = if cfg!(target_os = "windows") {
        PathBuf::from("..\\client\\Source\\Respark\\Private\\Generated")
    } else {
        PathBuf::from("../client/Source/Respark/Private/Generated")
    };
    std::fs::create_dir_all(&ue_cpp_out).ok();

    let status = Command::new("rebase")
        .args([
            "--input", data_dir.to_str().unwrap(),
            "--out-rust-file", rust_out_file.to_str().unwrap(),
            "--namespace", "Game::DB",
            "--out-ue", ue_cpp_out.to_str().unwrap(), // optional
        ])
        .status()
        .expect("failed to run rebase");
    assert!(status.success(), "rebase generation failed");

    println!("cargo:rustc-env=REBASE_RUST_FILE={}", rust_out_file.display());
}
```

Use in code:
```
pub mod rebasedb {
    // The generated file contains plain modules; no crate-level attributes.
    include!(env!("REBASE_RUST_FILE"));
}

use rebasedb::game::db::weapon::{get_weapon_data, find_weapon_by_name, WeaponKind};

fn main() {
    for row in get_weapon_data() {
        println!("Weapon {} DPS={}", row.name, row.dps);
    }
    if let Some(shotgun) = find_weapon_by_name("Shotgun2") {
        println!("Kind: {:?}", shotgun.kind);
    }
}
```

Notes:
- You can choose a stable path outside `OUT_DIR` if you prefer to check the file into VCS; then `include!("path/to/rebase_generated.rs")`.

--------------------------------------------------------------------------------

## Authoring tips

- Keep `type_name` PascalCase; field names will be sanitized for C++/Rust identifiers if needed.
- Use tuple forms for vectors/rotators when you want the simplest authoring: `(x, y, z)` and `(pitch, yaw, roll)`.
- If using map forms, quote the keys: `{ "x": 0.0, "y": 1.0, "z": 10.0 }`.
- Save files as UTF-8 without BOM to avoid parse issues.
- Use `--dry-run` to validate on CI without writing outputs.


--------------------------------------------------------------------------------



## Troubleshooting
- RON parse error like “expected map/tuple” or variant mismatch
  - Ensure `Vector`/`Rotator` values use tuples like `(0.0, 1.0, 2.0)` or quoted-key maps like `{ "x": 0.0, "y": 1.0, "z": 2.0 }`.
- Strange characters or parse fails at the first character
  - `.ron` file might be UTF-8 with BOM. Save as UTF-8 without BOM.
- UE link errors (unresolved external for `Get<Type>Data`/`Find<Type>ByName`)
  - Make sure the generated `.cpp` files live under your module’s `Source/<YourModule>/Private` so they are compiled. Copying only headers will cause linker errors.
- UE include errors for generated headers
  - If headers are under `Private/`, include them only from the same module. If other modules need them, move headers to `Public/` (keep `.cpp` in `Private/`).
- Enum values missing or unexpected
  - Enum variants are inferred from dataset strings; typos create new variants. Validate your records’ string values.
- build.rs cannot find the `rebase` executable
  - Install with `cargo install --path .` in your workspace or ensure `rebase` is on PATH in CI.

## Known limitations
- No UHT/UObject or `UPROPERTY`/`USTRUCT` generation yet; generated code is plain C++ (not exposed to Blueprint).
- Supported fields: scalars plus `Vector` and `Rotator`. Arrays/maps/optionals are not yet generated.
- `find_*` helpers are linear scan. For hot paths, build your own indices or request an optional index generator.
- Enum variants are inferred from data; enforce naming conventions in review if needed.
- Data is compiled into binaries; changing data requires rebuild (no runtime loader).
- Unreal Hot Reload may not reliably detect regenerated files; prefer a full recompile after schema changes.

## FAQ


- Why no UHT macros in UE output?
  - Plain C++ keeps generation simple and compile fast. If you need USTRUCT/UPROPERTY for Blueprint exposure, consider a future “USTRUCT mode”.

- How are enums generated?
  - Use `Enum(Name)` in your field `ty` and string values in records. ReBase converts them to PascalCase enumerators (digit-leading values are prefixed with `V`).

- How do I speed up lookup?
  - The generated APIs include linear `find_*` helpers. For hot paths, build your own hash maps at startup, or we can add an optional index generator if needed.

--------------------------------------------------------------------------------

## Contributing

- PRs welcome. Please include small sample `.ron` and expected snippets when possible.
- Keep changes modular and add tests where practical.

--------------------------------------------------------------------------------

## License

MIT