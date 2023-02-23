use std::path::Path;

use bevy::{
    pbr::wireframe::{Wireframe, WireframePlugin},
    prelude::*,
    render::{
        mesh::Indices,
        render_resource::PrimitiveTopology,
        settings::{WgpuFeatures, WgpuSettings},
    },
};
use smooth_bevy_cameras::{
    controllers::unreal::{UnrealCameraBundle, UnrealCameraController, UnrealCameraPlugin},
    LookTransformPlugin,
};
use vbsp::Bsp;

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        // .insert_resource(ClearColor(Color::rgb(0.1, 0.2, 0.3)))
        // .insert_resource(WgpuSettings {
        //     // Wireframe
        //     features: WgpuFeatures::POLYGON_MODE_LINE,
        //     ..Default::default()
        // })
        .add_plugins(DefaultPlugins)
        .add_plugin(LookTransformPlugin)
        .add_plugin(WireframePlugin)
        .add_plugin(UnrealCameraPlugin::default())
        .add_startup_system(setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // mut ambient_light: ResMut<AmbientLight>,
) {
    // ambient_light.color = Color::WHITE;
    // ambient_light.brightness = 0.05;

    // light
    // commands.spawn(PointLightBundle {
    //     point_light: PointLight {
    //         intensity: 1500.0,
    //         shadows_enabled: true,
    //         ..default()
    //     },
    //     transform: Transform::from_xyz(4.0, 8.0, 4.0),
    //     ..default()
    // });
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
        let data = std::fs::read("ex/ctf_2fort.bsp").unwrap();
        let bsp = Bsp::read(&data).unwrap();

        for model in bsp.models() {
            for face in model.faces() {
                if face.displacement().is_some() {
                    // TODO: implement
                    continue;
                }

                let texture = face.texture();
                let texture_data = texture.texture_data();

                if texture.flags.contains(vbsp::TextureFlags::NODRAW) {
                    continue;
                }
                // if !face.is_visible() {
                //     continue;
                // }

                let texture_name = texture.name();

                let reflect = texture_data.reflectivity;
                // println!("Reflect: {:#?}", reflect);
                let color = if texture.flags.contains(vbsp::TextureFlags::SKY) {
                    Color::rgb(0.0, 0.0, 0.0)
                } else {
                    if texture_name.eq_ignore_ascii_case("tools/toolstrigger") {
                        continue;
                    }

                    if !texture_name.contains("CONCRETE") {
                        println!("texture: {}", texture_name);
                    }

                    let alpha = if texture.flags.contains(vbsp::TextureFlags::TRANS) {
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

                let normal = if texture.flags.contains(vbsp::TextureFlags::SKY) {
                    [0.0, 0.0, 1.0]
                } else {
                    let plane = bsp.planes.get(face.plane_num as usize).unwrap();
                    plane.normal.into()
                };

                // let face_triangles = face
                //     .triangulate()
                //     .map(|tri| {
                //         [
                //             <[f32; 3]>::from(tri[0]),
                //             <[f32; 3]>::from(tri[1]),
                //             <[f32; 3]>::from(tri[2]),
                //         ]
                //     })
                //     .flatten()
                //     .collect::<Vec<_>>();
                // let face_normals = vec![normal; face_triangles.len()];

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
                    // Scale down the map
                    let scale = 0.1;
                    let vertex = [vertex[0] * scale, vertex[1] * scale, vertex[2] * scale];
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

                let material = StandardMaterial {
                    base_color: color,
                    alpha_mode: if color.a() < 1.0 {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    },
                    unlit: true,
                    metallic: 0.0,
                    reflectance: 0.0,
                    ..Default::default()
                };
                commands.spawn(PbrBundle {
                    mesh: meshes.add(mesh),
                    material: materials.add(material),
                    ..Default::default()
                });
            }
        }
    }
}

/// Rotate a right handed z-up (source engine) vector to a right handed y-up (bevy) vector.
fn rotate(v: [f32; 3]) -> [f32; 3] {
    [v[0], v[2], v[1]]
}
