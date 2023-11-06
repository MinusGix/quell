mod data;
pub mod map;

use std::{
    collections::HashSet,
    path::Path,
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    pbr::wireframe::{Wireframe, WireframePlugin},
    prelude::*,
    render::{
        mesh::Indices,
        render_resource::{
            Extent3d, PrimitiveTopology, Texture, TextureDescriptor, TextureDimension,
            TextureFormat, TextureUsages,
        },
        settings::{WgpuFeatures, WgpuSettings},
    },
};
use dashmap::DashSet;
use data::{LoadedTextures, VpkData, VpkState};
use image::DynamicImage;
use map::GameMap;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use smooth_bevy_cameras::{
    controllers::unreal::{UnrealCameraBundle, UnrealCameraController, UnrealCameraPlugin},
    LookTransform, LookTransformPlugin,
};
use vbsp::{Bsp, DisplacementInfo};

use crate::data::{
    construct_image, construct_material_info, construct_material_info2, GameId, LImage, LMaterial,
};

fn main() {
    // TODO: we should probably load vpks in setup so we can have a loading screen nicely
    let start_time = std::time::Instant::now();

    let game_id = GameId::Tf2;
    let root_path = "./ex/tf/";
    let vpk = VpkState::new(root_path, game_id).expect("Failed to load VPKs for the game");
    let loaded_textures = LoadedTextures::default();

    let end_time = std::time::Instant::now();
    println!("Loaded VPKs in {:?}", end_time - start_time);

    // use std::io::Write;
    // let root = &vpk.misc.data.tree;
    // let mut out_file = std::fs::File::create("out2.txt").unwrap();
    // for (key, v) in root {
    //     writeln!(out_file, "{}", key).unwrap();
    // }

    App::new()
        .insert_resource(Msaa::Sample4)
        // .insert_resource(ClearColor(Color::rgb(0.1, 0.2, 0.3)))
        // .insert_resource(WgpuSettings {
        //     // Wireframe
        //     features: WgpuFeatures::POLYGON_MODE_LINE,
        //     ..Default::default()
        // })
        .insert_resource(vpk)
        .insert_resource(loaded_textures)
        .add_plugins(DefaultPlugins)
        // .add_plugins(WireframePlugin)
        .add_plugins(LookTransformPlugin)
        .add_plugins(UnrealCameraPlugin::default())
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    // mut ambient_light: ResMut<AmbientLight>,
    mut asset_server: ResMut<AssetServer>,
    mut vpk: ResMut<VpkState>,
    mut loaded_textures: ResMut<LoadedTextures>,
    // mut map: Option<ResMut<GameMap>>,
) {
    println!("Setup");
    // ambient_light.color = Color::WHITE;
    // ambient_light.brightness = 0.05;

    // light
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 200.0, 4.0),
        ..default()
    });
    // camera
    // commands.spawn(Camera3dBundle {
    //     transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    //     ..default()
    // });
    commands
        .spawn(Camera3dBundle::default())
        .insert(UnrealCameraBundle::new(
            UnrealCameraController {
                keyboard_mvmt_sensitivity: 40.0,
                ..Default::default()
            },
            Vec3::new(-10.0, 5.0, 5.0),
            // Vec3::new(0., 0., 0.),
            // opposite direction
            Vec3::new(-35., 15., -10.),
            Vec3::Y,
        ));

    // let texture_handle = asset_server.load("out.png");

    // let quad_width = 8.0;
    // let quad_handle = meshes.add(Mesh::from(shape::Quad::new(Vec2::new(
    //     quad_width,
    //     quad_width * 0.25,
    // ))));

    // let material = materials.add(StandardMaterial {
    //     base_color_texture: Some(texture_handle),
    //     alpha_mode: AlphaMode::Blend,
    //     unlit: true,
    //     ..default()
    // });

    // commands.spawn(PbrBundle {
    //     mesh: quad_handle.clone(),
    //     material: material,
    //     transform: Transform::from_xyz(0.0, 0.0, 10.0),
    //     ..Default::default()
    // });

    {
        let map_path = "ex/ctf_2fort.bsp";
        // let map_path = "ex/tf/tf/maps/test.bsp";
        let map = GameMap::from_path(map_path).unwrap();

        load_materials(&mut vpk, &mut loaded_textures, &mut images, &map).unwrap();

        let start_time = std::time::Instant::now();

        // TODO: would it be better to do a Task based system likes the async_compute example?

        // TODO: We might be able to do something wacky and maybe more efficient by reserving
        // handles before hand.

        println!("Model count: #{}", map.bsp.models.len());

        let cmds = map
            .bsp
            .models
            .par_iter()
            .flat_map(|m| {
                let start = m.first_face as usize;
                let end = start + m.face_count as usize;

                map.bsp.faces[start..end].par_iter()
            })
            .filter_map(|face| {
                let face = vbsp::Handle::new(&map.bsp, face);
                let res = construct_face_cmd(&loaded_textures, &map, face).transpose()?;
                match res {
                    Ok(face_info) => Some(face_info),
                    Err(err) => {
                        eprintln!("Failed to construct face: {:?}", err);
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(move |face_info| {
                let mesh = meshes.add(face_info.mesh);
                let material = materials.add(face_info.material);

                Some(PbrBundle {
                    mesh,
                    material,
                    ..Default::default()
                })
            })
            // We have to collect a second time because spawn_batch requires a 'static
            // iterator
            .collect::<Vec<_>>();

        commands.spawn_batch(cmds);

        let end_time = std::time::Instant::now();

        println!("Loaded map in {:?}", end_time - start_time);

        commands.insert_resource(map);
    }
}

/// Get all of the names of the materials (vmts) that are referenced in the map.  
/// These names are deduplicated.
fn material_names(map: &GameMap) -> Vec<Arc<str>> {
    let start_time = std::time::Instant::now();

    // Ex: ctf_2fort has 227 unique materials referenced (directly)
    let mut material_names = Vec::with_capacity(200);
    let mut texture_name_indices = HashSet::with_capacity(300);
    let mut face_count = 0;
    let mut prob_vis_face_count = 0;
    for model in map.bsp.models() {
        for face in model.faces() {
            face_count += 1;

            let texture_info = face.texture();

            if texture_info.flags.contains(vbsp::TextureFlags::NODRAW) {
                continue;
            } else if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
                continue;
            }

            let material_name = texture_info.name();
            if material_name.eq_ignore_ascii_case("tools/toolstrigger") {
                continue;
            }

            if let Some(_disp) = face.displacement() {
                // TODO: displacements have textures too!
                continue;
            }

            prob_vis_face_count += 1;

            let texture_name_index = texture_info.texture_data().name_string_table_id;

            if !texture_name_indices.insert(texture_name_index) {
                continue;
            }

            // TODO: should we to_lowercase them?
            material_names.push(Arc::from(material_name));
        }
    }

    let end_time = std::time::Instant::now();
    println!(
        "Loaded #{} material names in {:?}\n\tFace count: {prob_vis_face_count} / {face_count}",
        material_names.len(),
        end_time - start_time
    );

    material_names
}

/// Load all the (materials -> textures) as images in parallel
fn load_materials(
    vpk: &mut VpkState,
    loaded_textures: &mut LoadedTextures,
    images: &mut Assets<Image>,
    map: &GameMap,
) -> eyre::Result<()> {
    let material_names = material_names(map);

    let start_time = std::time::Instant::now();

    let vpk2 = &*vpk;

    let duplicate_counts = AtomicUsize::new(0);

    // The loaded/loading textures
    let l: DashSet<Arc<str>> = DashSet::with_capacity(material_names.len());
    // Iterate over all the materials, loading them.
    // Typically, none of the materials will error at all so the size is usually the same as
    // `material_names`
    let iter = material_names
        .into_par_iter()
        .filter_map(move |material_name| {
            match construct_material_info2(vpk2, Some(map), &material_name) {
                Ok(info) => Some((material_name, info)),
                Err(err) => {
                    eprintln!(
                        "Failed to construct material info for {}: {:?}",
                        material_name, err
                    );
                    None
                }
            }
        })
        // Check if we need to be the instance loading the texture
        .map(|(material_name, info)| {
            if l.contains(&info.base_texture_name) {
                duplicate_counts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return (material_name, info, false);
            }

            l.insert(info.base_texture_name.clone());

            (material_name, info, true)
        })
        .filter_map(|(material_name, info, should_load_img)| {
            if !should_load_img {
                return Some((material_name, info, None));
            }

            let res = construct_image(vpk2, Some(map), &info.base_texture_name);
            match res {
                Ok((image, img_src)) => Some((material_name, info, Some((image, img_src)))),
                Err(err) => {
                    eprintln!(
                        "Failed to construct image for material {}, texture {}: {:?}",
                        material_name, info.base_texture_name, err
                    );
                    None
                }
            }
        })
        .collect::<Vec<_>>();

    for (material_name, info, image) in iter {
        if let Some((image, img_src)) = image {
            loaded_textures.insert_texture_of(
                images,
                info.base_texture_name.clone(),
                image,
                img_src,
            )?;
        }

        let material = LMaterial {
            image: Ok(info.base_texture_name),
            vmt_src: info.vmt_src,
        };

        loaded_textures.insert_material(material_name, material);
    }

    let end_time = std::time::Instant::now();

    println!("Loaded textures in {:?}", end_time - start_time);
    println!(
        "Duplicates: {}",
        duplicate_counts.load(std::sync::atomic::Ordering::SeqCst)
    );

    loaded_textures.frozen = true;

    Ok(())
}

const SCALE: f32 = 0.1;

struct FaceInfo {
    mesh: Mesh,
    material: StandardMaterial,
}

fn construct_face_cmd(
    loaded_textures: &LoadedTextures,
    map: &GameMap,
    face: vbsp::Handle<'_, vbsp::Face>,
) -> eyre::Result<Option<FaceInfo>> {
    let texture_info = face.texture();
    let texture_data = texture_info.texture_data();

    if texture_info.flags.contains(vbsp::TextureFlags::NODRAW) {
        // TODO: create nodraw meshes but hide them so we can render them in debug mode
        return Ok(None);
    } else if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
        // TODO: create the skybox
        return Ok(None);
    }

    let texture_name = texture_info.name();

    // TODO: this probably ignores other pieces that we shouldn't render
    if texture_name.eq_ignore_ascii_case("tools/toolstrigger") {
        return Ok(None);
    }

    let reflect = texture_data.reflectivity;
    let color = if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
        Color::rgb(0.0, 0.0, 0.0)
    } else {
        let alpha = if texture_info.flags.contains(vbsp::TextureFlags::TRANS) {
            0.2
        } else {
            1.0
        };

        // TODO: actually just get this texture
        if texture_name == "water/water_2fort_beneath.vmt" {
            Color::rgba(0.0, 0.0, 0.8, 0.4)
        } else {
            Color::rgba(reflect.x, reflect.y, reflect.z, alpha)
        }
    };

    if let Some(disp) = face.displacement() {
        let (mesh, material) = create_displacement_mesh(&map.bsp, face, disp, color);

        Ok(Some(FaceInfo { mesh, material }))
    } else {
        let texture_name = texture_info.name();
        let texture = loaded_textures
            .find_material_texture(texture_name)
            .unwrap()
            .unwrap();

        let (mesh, material) = create_basic_map_mesh(&map.bsp, face, color, Some(texture));

        Ok(Some(FaceInfo { mesh, material }))
    }
}

// TODO: we don't create the map mesh with a transform yet it is positioned fine, so probably the
// triangles are being positioned 'manually'. I think we should maybe try getting them to center on
// 0,0,0 and then apply a transform to make it work nicer with other transform stuff
fn create_basic_map_mesh<'a>(
    bsp: &'a Bsp,
    face: vbsp::Handle<'a, vbsp::Face>,
    color: Color,
    texture: Option<Handle<Image>>,
) -> (Mesh, StandardMaterial) {
    let texture_info = face.texture();
    let tex_width = texture_info.texture().width as f32;
    let tex_height = texture_info.texture().height as f32;

    let normal = if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
        [0.0, 0.0, 1.0]
    } else {
        let plane = bsp.planes.get(face.plane_num as usize).unwrap();
        plane.normal.into()
    };
    // let normal = [-normal[0], -normal[2], -normal[1]];
    // TODO: do we need to rotate the normal?

    // TODO(minor): preallocate
    let mut face_triangles = Vec::new();
    let mut face_normals = Vec::new();
    let mut face_uvs = Vec::new();

    let mut triangle_vert = 0;
    let mut triangle = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
    for i in 0..face.num_edges {
        let surface_edge = bsp
            .surface_edges
            .get((face.first_edge + i as i32) as usize)
            .unwrap();
        let edge = bsp.edges.get(surface_edge.edge_index() as usize).unwrap();
        let vertex_index = match surface_edge.direction() {
            vbsp::EdgeDirection::FirstToLast => edge.start_index,
            vbsp::EdgeDirection::LastToFirst => edge.end_index,
        };

        let vertex = bsp.vertices.get(vertex_index as usize).unwrap();
        let vertex = <[f32; 3]>::from(vertex.position);
        let vertex = [vertex[0] * SCALE, vertex[1] * SCALE, vertex[2] * SCALE];
        let vertex = rotate(vertex);

        triangle[triangle_vert] = vertex;
        triangle_vert += 1;

        if triangle_vert > 2 {
            // TODO: I swapped the order of these because my rotate also made the z neg
            // and that seems to fix things, but I don't completely understand the details
            let vert = triangle[2];
            face_triangles.push(vert);
            face_normals.push(normal);
            face_uvs.push(calc_uv(&texture_info, vert, tex_width, tex_height));

            let vert = triangle[1];
            face_triangles.push(vert);
            face_normals.push(normal);
            face_uvs.push(calc_uv(&texture_info, vert, tex_width, tex_height));

            let vert = triangle[0];
            face_triangles.push(vert);
            face_normals.push(normal);
            face_uvs.push(calc_uv(&texture_info, vert, tex_width, tex_height));

            triangle[1] = triangle[2];
            triangle_vert = 2;
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, face_triangles);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, face_normals);
    // panic!();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, face_uvs);
    // TODO: lightmaps with UV_1?

    // Create the material
    let material = StandardMaterial {
        // base_color: color,
        base_color_texture: texture.clone(),
        // alpha_mode: AlphaMode::Blend,
        // unlit: true,
        // emissive_texture: texture,
        alpha_mode: if color.a() < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        // TODO: might be needed since source uses DX
        // flip_normal_map_y

        // unlit: true,
        // metallic: 0.0,
        // emissive: color,
        // unlit: true,
        // reflectance: 0.0,
        ..Default::default()
    };

    (mesh, material)
}

/// Calculate the UV coordinates for the given vertex and texture.
fn calc_uv(
    texture_info: &vbsp::TextureInfo,
    vertex: [f32; 3],
    tex_width: f32,
    tex_height: f32,
) -> [f32; 2] {
    // [xmul, ymul, zmul, offset]
    let scale = texture_info.texture_scale;
    let transform = texture_info.texture_transform;

    // Undo the scaling
    let vertex = [vertex[0] / SCALE, vertex[1] / SCALE, vertex[2] / SCALE];
    // Convert to texture coordinates (y-down)
    let vertex = tex_coord(vertex);

    // Convert from z-up to y-up, and then to texture coordinates
    let scale = tex_coord_4(rotate_4(scale));
    let transform = tex_coord_4(rotate_4(transform));

    // xmul * x + ymul * y + zmul * z + offset
    let u = scale[0] * vertex[0] + scale[1] * vertex[1] + scale[2] * vertex[2] + scale[3];
    let v = transform[0] * vertex[0]
        + transform[1] * vertex[1]
        + transform[2] * vertex[2]
        + transform[3];

    // Normalize by the texture size
    let u = u / tex_width;
    let v = v / tex_height;

    [u, v]
}

fn create_displacement_mesh<'a>(
    bsp: &'a vbsp::Bsp,
    face: vbsp::Handle<'a, vbsp::Face>,
    disp: vbsp::Handle<'a, DisplacementInfo>,
    color: Color,
) -> (Mesh, StandardMaterial) {
    let low_base = disp.start_position; // * SCALE;
    let low_base = <[f32; 3]>::from(low_base);
    // let low_base = rotate(low_base);

    if face.num_edges != 4 {
        panic!("Bad displacement!\n");
    }

    let mut corner_verts = [[0.0, 0.0, 0.0]; 4];
    let mut base_i = None;
    let mut base_dist = std::f32::INFINITY;
    for i in 0..4 {
        let surface_edge = bsp
            .surface_edges
            .get((face.first_edge + i as i32) as usize)
            .unwrap();
        let edge = bsp.edges.get(surface_edge.edge_index() as usize).unwrap();
        let vertex_index = match surface_edge.direction() {
            vbsp::EdgeDirection::FirstToLast => edge.start_index,
            vbsp::EdgeDirection::LastToFirst => edge.end_index,
        };

        let vertex = bsp.vertices.get(vertex_index as usize).unwrap();
        let vertex = <[f32; 3]>::from(vertex.position);
        // let vertex = [vertex[0] * SCALE, vertex[1] * SCALE, vertex[2] * SCALE];
        // let vertex = rotate(vertex);

        corner_verts[i] = vertex;

        let this_dist = (vertex[0] - low_base[0]).abs()
            + (vertex[2] - low_base[2]).abs()
            + (vertex[1] - low_base[1]).abs();
        if this_dist < base_dist {
            base_dist = this_dist;
            base_i = Some(i);
        }
    }

    let base_i = base_i.expect("Bad base in displacement");

    let high_base = corner_verts[(base_i + 3) % 4];
    let high_ray = corner_verts[(base_i + 2) % 4];
    let high_ray = [
        high_ray[0] - high_base[0],
        high_ray[1] - high_base[1],
        high_ray[2] - high_base[2],
    ];
    let low_ray = corner_verts[(base_i + 1) % 4];
    let low_ray = [
        low_ray[0] - low_base[0],
        low_ray[1] - low_base[1],
        low_ray[2] - low_base[2],
    ];

    let verts_wide = (2 << (disp.power - 1)) + 1;

    let mut base_verts = vec![[0.0, 0.0, 0.0]; 289];
    let mut base_alphas = vec![0.0; 289];

    for y in 0..verts_wide {
        let fy = y as f32 / (verts_wide as f32 - 1.0);

        let mid_base = [
            low_base[0] + low_ray[0] * fy,
            low_base[1] + low_ray[1] * fy,
            low_base[2] + low_ray[2] * fy,
        ];
        let mid_ray = [
            high_base[0] + high_ray[0] * fy - mid_base[0],
            high_base[1] + high_ray[1] * fy - mid_base[1],
            high_base[2] + high_ray[2] * fy - mid_base[2],
        ];

        for x in 0..verts_wide {
            let fx = x as f32 / (verts_wide as f32 - 1.0);
            let i = x + y * verts_wide;

            // TODO: use disp.displacement_vertices
            let disp_vert = bsp
                .displacement_vertices
                .get((disp.displacement_vertex_start + i) as usize)
                .unwrap();
            let offset = <[f32; 3]>::from(disp_vert.vector);
            let scale = disp_vert.distance;
            let alpha = disp_vert.alpha;

            base_verts[i as usize] = [
                mid_base[0] + mid_ray[0] * fx + offset[0] * scale,
                mid_base[1] + mid_ray[1] * fx + offset[1] * scale,
                mid_base[2] + mid_ray[2] * fx + offset[2] * scale,
            ];
            base_alphas[i as usize] = alpha;
        }
    }

    let mut tris = Vec::new();
    let mut normals = Vec::new();

    for y in 0..(verts_wide - 1) {
        for x in 0..(verts_wide - 1) {
            let i = x + y * verts_wide;

            // TODO: Displacement don't work with the other rotate, why?
            // They look fine with just swapping the z and y, but the normal mesh
            // looked fine with that until I had to apply the texture.
            let v1 = scale(rotate_s(base_verts[i as usize]));
            let v2 = scale(rotate_s(base_verts[(i + 1) as usize]));
            let v3 = scale(rotate_s(base_verts[(i + verts_wide) as usize]));
            let v4 = scale(rotate_s(base_verts[(i + verts_wide + 1) as usize]));

            if i % 2 != 0 {
                let normal = find_normal(v1, v3, v2);
                // let color = Color::rgb(tex_r1, tex_g1, tex_b1);

                tris.push(v1);
                normals.push(normal);
                tris.push(v3);
                normals.push(normal);
                tris.push(v2);
                normals.push(normal);

                let normal = find_normal(v2, v3, v4);

                tris.push(v2);
                normals.push(normal);
                tris.push(v3);
                normals.push(normal);
                tris.push(v4);
                normals.push(normal);
            } else {
                let normal = find_normal(v1, v3, v4);

                tris.push(v1);
                normals.push(normal);
                tris.push(v3);
                normals.push(normal);
                tris.push(v4);
                normals.push(normal);

                let normal = find_normal(v1, v4, v2);

                tris.push(v2);
                normals.push(normal);
                tris.push(v1);
                normals.push(normal);
                tris.push(v4);
                normals.push(normal);
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, tris);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

    let material: StandardMaterial = color.into();

    (mesh, material)
}

fn find_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - c[0], b[1] - c[1], b[2] - c[2]];
    let v = [a[0] - c[0], a[1] - c[1], a[2] - c[2]];

    let norm = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];

    let len = (norm[0] * norm[0] + norm[1] * norm[1] + norm[2] * norm[2]).sqrt();

    [norm[0] / len, norm[1] / len, norm[2] / len]
}

// fn pick_color(name: &str, x: f32) -> u32 {
//     // TODO: more colors
//     let col = 77550;

//     col
// }

/// Rotate a right handed z-up (source engine) vector to a right handed y-up (bevy) vector.
fn rotate(v: [f32; 3]) -> [f32; 3] {
    [v[0], v[2], -v[1]]
}
fn rotate_s(v: [f32; 3]) -> [f32; 3] {
    [v[0], v[2], v[1]]
}
/// Rotate a right handed z-up (source engine) vector to a right handed y-up (bevy) vector.
fn rotate_4(v: [f32; 4]) -> [f32; 4] {
    [v[0], v[2], -v[1], v[3]]
}
fn scale(v: [f32; 3]) -> [f32; 3] {
    [v[0] * SCALE, v[1] * SCALE, v[2] * SCALE]
}
/// Convert a y-up (bevy) vector to a tex coordinate vector
fn tex_coord(v: [f32; 3]) -> [f32; 3] {
    [v[0], -v[1], v[2]]
}
fn tex_coord_4(v: [f32; 4]) -> [f32; 4] {
    [v[0], -v[1], v[2], v[3]]
}
