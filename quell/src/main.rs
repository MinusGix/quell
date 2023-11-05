mod data;
pub mod map;

use std::path::Path;

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
    utils::HashMap,
};
use data::{LoadedTextures, VpkData, VpkState};
use image::DynamicImage;
use map::GameMap;
use smooth_bevy_cameras::{
    controllers::unreal::{UnrealCameraBundle, UnrealCameraController, UnrealCameraPlugin},
    LookTransformPlugin,
};
use vbsp::{Bsp, DisplacementInfo};

fn main() {
    let vpk = VpkState::new();
    let loaded_textures = LoadedTextures::default();

    use std::io::Write;
    let root = &vpk.misc.data.tree;
    let mut out_file = std::fs::File::create("out2.txt").unwrap();
    for (key, v) in root {
        writeln!(out_file, "{}", key).unwrap();
    }

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
        // .insert_resource(None::<GameMap>)
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
    mut vpk: ResMut<VpkState>,
    mut loaded_textures: ResMut<LoadedTextures>,
    // mut map: Option<ResMut<GameMap>>,
) {
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
            UnrealCameraController::default(),
            Vec3::new(-2.0, 5.0, 5.0),
            Vec3::new(0., 0., 0.),
            Vec3::Y,
        ));

    {
        // let data = std::fs::read("ex/ctf_2fort.bsp").unwrap();
        // let bsp = Bsp::read(&data).unwrap();
        // map = Some(GameMap::from_path("ex/ctf_2fort.bsp").unwrap());

        // commands.insert_resource(GameMap::from_path("ex/ctf_2fort.bsp").unwrap());
        // let map = map.as_ref().unwrap();
        let map = GameMap::from_path("ex/ctf_2fort.bsp").unwrap();

        // {
        //     let mut out_file = std::fs::File::create("map_out.txt").unwrap();
        //     let zip = map.bsp.pack.zip.lock().unwrap();
        //     use std::io::Write;
        //     for name in zip.file_names() {
        //         writeln!(out_file, "{}", name).unwrap();
        //     }
        // };

        for model in map.bsp.models() {
            for face in model.faces() {
                let texture_info = face.texture();
                let texture_data = texture_info.texture_data();

                if texture_info.flags.contains(vbsp::TextureFlags::NODRAW) {
                    continue;
                } else if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
                    continue;
                }

                let texture_name = texture_info.name();

                let reflect = texture_data.reflectivity;
                let color = if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
                    Color::rgb(0.0, 0.0, 0.0)
                } else {
                    if texture_name.eq_ignore_ascii_case("tools/toolstrigger") {
                        continue;
                    }

                    let alpha = if texture_info.flags.contains(vbsp::TextureFlags::TRANS) {
                        0.2
                    } else {
                        1.0
                    };

                    // TODO: actually just get this texture
                    if texture_name == "water/water_2fort_beneath.vmt" {
                        Color::rgba(0.0, 0.0, 0.8, 0.4)
                    } else {
                        // Color::rgba(reflect.x.sqrt(), reflect.y.sqrt(), reflect.z.sqrt(), alpha)
                        Color::rgba(reflect.x, reflect.y, reflect.z, alpha)
                        // Color::RED
                    }
                };

                if let Some(disp) = face.displacement() {
                    let (mesh, material) = create_displacement_mesh(&map.bsp, face, disp, color);

                    commands.spawn(PbrBundle {
                        mesh: meshes.add(mesh),
                        material: materials.add(material),
                        ..Default::default()
                    });
                } else {
                    let texture_name = texture_info.name();
                    // let texture = vpk.load_texture(&mut images, texture_name);
                    let texture = loaded_textures.load_texture(
                        &mut vpk,
                        Some(&map),
                        &mut images,
                        texture_name,
                    );

                    let (mesh, material) = create_basic_map_mesh(&map.bsp, face, color, texture);

                    commands.spawn(PbrBundle {
                        mesh: meshes.add(mesh),
                        material: materials.add(material),
                        ..Default::default()
                    });
                }
            }
        }

        commands.insert_resource(map);
    }
}

const SCALE: f32 = 0.1;

fn create_basic_map_mesh<'a>(
    bsp: &'a Bsp,
    face: vbsp::Handle<'a, vbsp::Face>,
    color: Color,
    texture: Option<Handle<Image>>,
) -> (Mesh, StandardMaterial) {
    let texture_info = face.texture();

    let normal = if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
        [0.0, 0.0, 1.0]
    } else {
        let plane = bsp.planes.get(face.plane_num as usize).unwrap();
        plane.normal.into()
    };

    let mut face_triangles = Vec::new();
    let mut face_normals = Vec::new();

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

        // face_triangles.push(vertex);
        triangle[triangle_vert] = vertex;
        triangle_vert += 1;

        if triangle_vert > 2 {
            let vert = triangle[0];
            face_triangles.push(vert);
            face_normals.push(normal);

            let vert = triangle[1];
            face_triangles.push(vert);
            face_normals.push(normal);

            let vert = triangle[2];
            face_triangles.push(vert);
            face_normals.push(normal);

            triangle[1] = triangle[2];
            triangle_vert = 2;
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, face_triangles);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, face_normals);

    // Create the material
    let material = StandardMaterial {
        base_color: color,
        base_color_texture: texture,
        alpha_mode: if color.a() < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        // unlit: true,
        // metallic: 0.0,
        emissive: color,
        // reflectance: 0.0,
        ..Default::default()
    };

    (mesh, material)
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
    // let texture_name = face.texture().name();

    for y in 0..(verts_wide - 1) {
        for x in 0..(verts_wide - 1) {
            let i = x + y * verts_wide;

            let v1 = scale(rotate(base_verts[i as usize]));
            let v2 = scale(rotate(base_verts[(i + 1) as usize]));
            let v3 = scale(rotate(base_verts[(i + verts_wide) as usize]));
            let v4 = scale(rotate(base_verts[(i + verts_wide + 1) as usize]));

            // let color1 = pick_color(texture_name, base_alphas[i as usize]);

            // let tex_r1 = ((color1 >> 16) & 0xFF) as f32 / 255.0;
            // let tex_g1 = ((color1 >> 8) & 0xFF) as f32 / 255.0;
            // let tex_b1 = (color1 & 0xFF) as f32 / 255.0;

            // let color2 = pick_color(texture_name, base_alphas[(i + 1) as usize]);

            // let tex_r2 = ((color2 >> 16) & 0xFF) as f32 / 255.0;
            // let tex_g2 = ((color2 >> 8) & 0xFF) as f32 / 255.0;
            // let tex_b2 = (color2 & 0xFF) as f32 / 255.0;

            // let color3 = pick_color(texture_name, base_alphas[(i + verts_wide) as usize]);

            // let tex_r3 = ((color3 >> 16) & 0xFF) as f32 / 255.0;
            // let tex_g3 = ((color3 >> 8) & 0xFF) as f32 / 255.0;
            // let tex_b3 = (color3 & 0xFF) as f32 / 255.0;

            // let color4 = pick_color(texture_name, base_alphas[(i + verts_wide + 1) as usize]);

            // let tex_r4 = ((color4 >> 16) & 0xFF) as f32 / 255.0;
            // let tex_g4 = ((color4 >> 8) & 0xFF) as f32 / 255.0;
            // let tex_b4 = (color4 & 0xFF) as f32 / 255.0;

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
    [v[0], v[2], v[1]]
}
fn scale(v: [f32; 3]) -> [f32; 3] {
    [v[0] * SCALE, v[1] * SCALE, v[2] * SCALE]
}
