//! Optional integration verification. No game content is embedded in tests.
use std::{fs, path::Path};

#[test]
#[ignore = "requires local assets/imported; run cargo test --test imported_assets -- --ignored"]
fn local_glbs_have_valid_geometry_textures_skins_and_animation() {
    fn visit(path: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "glb") {
                files.push(path);
            }
        }
    }
    let mut files = Vec::new();
    visit(Path::new("assets/imported"), &mut files);
    assert!(!files.is_empty());
    for path in files {
        let bytes = fs::read(&path).unwrap();
        let asset = gltf::Gltf::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let blob = asset.blob.as_deref().unwrap();
        let sidecar: serde_json::Value =
            serde_json::from_slice(&fs::read(path.with_extension("import.json")).unwrap()).unwrap();
        assert!(
            asset.meshes().count() > 0,
            "{} has no meshes",
            path.display()
        );
        for mesh in asset.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|_| Some(blob));
                let positions: Vec<_> = reader.read_positions().unwrap().collect();
                assert!(positions.iter().flatten().all(|v| v.is_finite()));
                let indices: Vec<_> = reader.read_indices().unwrap().into_u32().collect();
                assert!(indices.iter().all(|&i| (i as usize) < positions.len()));
                assert!(indices.len().is_multiple_of(3));
                assert!(reader.read_tex_coords(0).is_some());
                assert!(
                    primitive
                        .material()
                        .pbr_metallic_roughness()
                        .base_color_texture()
                        .is_some()
                );
            }
        }
        if sidecar["kind"] != "map" {
            assert_eq!(asset.skins().count(), 1);
            assert_eq!(
                asset.animations().count(),
                sidecar["animations"].as_array().unwrap().len()
            );
            let skin = asset.skins().next().unwrap();
            let joints = skin.joints().count();
            assert_eq!(
                skin.reader(|_| Some(blob))
                    .read_inverse_bind_matrices()
                    .unwrap()
                    .count(),
                joints
            );
            let mut moving = false;
            for animation in asset.animations() {
                for channel in animation.channels() {
                    let reader = channel.reader(|_| Some(blob));
                    let times: Vec<_> = reader.read_inputs().unwrap().collect();
                    assert!(times.windows(2).all(|t| t[1] > t[0]));
                    if let Some(gltf::animation::util::ReadOutputs::Rotations(rotations)) =
                        reader.read_outputs()
                    {
                        let values: Vec<_> = rotations.into_f32().collect();
                        moving |= values.windows(2).any(|v| v[0] != v[1]);
                        assert!(
                            values
                                .iter()
                                .all(|q| (q.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 1e-3)
                        );
                    }
                }
            }
            assert!(moving, "{} has no animated joint motion", path.display());
        }
    }
}
