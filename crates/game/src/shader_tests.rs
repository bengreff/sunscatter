//! Every shader of ours composes, validates and compiles to Metal's
//! language against Bevy's shader libraries, as Bevy does at run time
//! (naga_oil, naga), with the shader defs Bevy's pipelines set. Otherwise a
//! shader error shows only when the game runs, and some not even then: an
//! import naga_oil cannot resolve is taken by Bevy for one not loaded yet,
//! and the pipeline waits for ever, drawing nothing (the terrain's struct
//! field `ambient` against the imported module `ambient`, 2026-10-26). And
//! the structs the shaders read have the layout of the Rust types that
//! fill them.

use bevy::render::render_resource::ShaderType;
use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue, ShaderLanguage, ShaderType as Stage,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Bevy's crates whose shaders ours import.
const BEVY_CRATES: [&str; 7] = [
    "bevy_pbr",
    "bevy_render",
    "bevy_core_pipeline",
    "bevy_light",
    "bevy_shader",
    "bevy_post_process",
    "bevy_anti_alias",
];

fn game_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The source directories of Bevy's crates, from `cargo metadata`.
fn bevy_src_dirs() -> Vec<PathBuf> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let out = std::process::Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--offline", "--manifest-path"])
        .arg(&manifest)
        .output()
        .expect("cargo metadata runs");
    assert!(out.status.success(), "cargo metadata: {}", String::from_utf8_lossy(&out.stderr));
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).expect("metadata is JSON");
    meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter(|p| BEVY_CRATES.contains(&p["name"].as_str().unwrap_or("")))
        .filter_map(|p| Some(Path::new(p["manifest_path"].as_str()?).parent()?.join("src")))
        .collect()
}

fn wgsl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            wgsl_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "wgsl") {
            out.push(p);
        }
    }
}

/// A composer holding every importable module (Bevy's and ours), added in
/// dependency order, under `defs`.
fn composer(defs: &HashMap<String, ShaderDefValue>) -> Composer {
    let mut files = Vec::new();
    for dir in bevy_src_dirs() {
        wgsl_files(&dir, &mut files);
    }
    wgsl_files(&game_src(), &mut files);
    let mut modules: Vec<(PathBuf, String)> = files
        .into_iter()
        .filter_map(|p| {
            let s = std::fs::read_to_string(&p).ok()?;
            s.contains("#define_import_path").then_some((p, s))
        })
        .collect();
    assert!(modules.len() > 50, "Bevy's shader libraries found: {}", modules.len());
    let mut c = Composer::default().with_capabilities(naga::valid::Capabilities::all());
    loop {
        let before = modules.len();
        modules.retain(|(p, s)| {
            c.add_composable_module(ComposableModuleDescriptor {
                source: s,
                file_path: &p.to_string_lossy(),
                language: ShaderLanguage::Wgsl,
                as_name: None,
                additional_imports: &[],
                shader_defs: defs.clone(),
            })
            .is_err()
        });
        if modules.is_empty() || modules.len() == before {
            break;
        }
    }
    c
}

/// Bevy's mesh pipeline's defs for our views (MSAA, the atmosphere, a
/// perspective camera, Gaussian shadow filtering) and the material group.
fn mesh_defs() -> HashMap<String, ShaderDefValue> {
    let mut d: HashMap<String, ShaderDefValue> = [
        "MESH_PIPELINE",
        "VERTEX_OUTPUT_INSTANCE_INDEX",
        "VERTEX_POSITIONS",
        "VERTEX_NORMALS",
        "VERTEX_UVS",
        "VERTEX_UVS_A",
        "VERTEX_COLORS",
        "MULTISAMPLED",
        "VIEW_PROJECTION_PERSPECTIVE",
        "SHADOW_FILTER_METHOD_GAUSSIAN",
        "ATMOSPHERE",
        "IRRADIANCE_VOLUMES_ARE_USABLE",
        "DUAL_SOURCE_BLENDING",
    ]
    .into_iter()
    .map(|k| (k.to_string(), ShaderDefValue::Bool(true)))
    .collect();
    for (k, v) in [
        ("MATERIAL_BIND_GROUP", 3),
        ("MAX_DIRECTIONAL_LIGHTS", 10),
        ("MAX_CASCADES_PER_LIGHT", 4),
        ("MAX_RECT_LIGHTS", 4),
        ("AVAILABLE_STORAGE_BUFFER_BINDINGS", 8),
    ] {
        d.insert(k.into(), ShaderDefValue::UInt(v));
    }
    d.insert("SCREEN_SPACE_SPECULAR_TRANSMISSION_BLUR_TAPS".into(), ShaderDefValue::Int(4));
    d
}

/// Composes `file` (relative to the game's src), validates it and writes
/// it as Metal shading language; the module, for layout checks. With
/// `SUNSCATTER_DUMP_MSL=<dir>` the Metal source is saved there (to read
/// what a shader compiles to).
fn compile(c: &mut Composer, file: &str, defs: &HashMap<String, ShaderDefValue>) -> naga::Module {
    let path = game_src().join(file);
    let source = std::fs::read_to_string(&path).expect("shader source");
    let module = c
        .make_naga_module(NagaModuleDescriptor {
            source: &source,
            file_path: &path.to_string_lossy(),
            shader_type: Stage::Wgsl,
            shader_defs: defs.clone(),
            additional_imports: &[],
        })
        .unwrap_or_else(|e| panic!("{file} does not compose:\n{}", e.emit_to_string(c)));
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{file} does not validate: {:?}", e.into_inner()));
    let options = naga::back::msl::Options { lang_version: (2, 4), ..Default::default() };
    let (msl, _) =
        naga::back::msl::write_string(&module, &info, &options, &naga::back::msl::PipelineOptions::default())
            .unwrap_or_else(|e| panic!("{file} does not compile to MSL: {e:?}"));
    if let Ok(dir) = std::env::var("SUNSCATTER_DUMP_MSL") {
        let _ = std::fs::write(Path::new(&dir).join(file.replace('/', "_") + ".metal"), msl);
    }
    module
}

/// The byte size naga lays struct `name` out with.
fn span(module: &naga::Module, name: &str) -> u32 {
    module
        .types
        .iter()
        .find_map(|(_, t)| match &t.inner {
            naga::TypeInner::Struct { span, .. } if t.name.as_deref() == Some(name) => Some(*span),
            _ => None,
        })
        .unwrap_or_else(|| panic!("struct {name} not found"))
}

#[test]
fn every_shader_composes_validates_and_compiles_for_metal() {
    let defs = mesh_defs();
    let mut c = composer(&defs);
    for file in [
        "terrain/terrain.wgsl",
        "ship.wgsl",
        "rings.wgsl",
        "clouds.wgsl",
        "sky/stars.wgsl",
        "sky/render_sky.wgsl",
        "sky/aerial_view_lut.wgsl",
        "upscale.wgsl",
    ] {
        compile(&mut c, file, &defs);
    }
}

#[test]
fn the_shaders_structs_match_the_rust_types() {
    let defs = mesh_defs();
    let mut c = composer(&defs);
    let terrain = compile(&mut c, "terrain/terrain.wgsl", &defs);
    // (struct, naga's size, the Rust type's)
    let cases = [
        ("TerrainFrame", span(&terrain, "TerrainFrame"), crate::terrain::TerrainFrame::min_size().get()),
        ("TerrainParams", span(&terrain, "TerrainParams"), crate::terrain::TerrainParams::min_size().get()),
        (
            "ShipLight",
            span(&compile(&mut c, "ship.wgsl", &defs), "ShipLight"),
            crate::ship_light::ShipLight::min_size().get(),
        ),
    ];
    for (name, shader, rust) in cases {
        assert_eq!(u64::from(shader), rust, "{name}");
    }
}
