use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};

use bevy_mod_outline::OutlinePlugin;
use quell::{
    conf::{Config, MatLeafvis},
    data::{GameId, LoadedTextures, VpkState},
    map::GameMap,
    material::load_materials,
    mesh::{construct_meshes, rotate, rotate_s, scale, unrotate, unscale, FaceInfo},
    util::transform_to_vbsp,
};

use rayon::prelude::ParallelIterator;
use smooth_bevy_cameras::{
    controllers::unreal::{UnrealCameraBundle, UnrealCameraController, UnrealCameraPlugin},
    LookTransformPlugin,
};

fn main() {
    // TODO: we should probably load vpks in setup so we can have a loading screen nicely
    let start_time = std::time::Instant::now();

    let mut conf = Config::default();
    // TODO: load console commands from file or allow them via cli

    conf.render.mat.leafvis = MatLeafvis::CurrentVisleaf;
    conf.render.no_vis = true;

    let game_id = GameId::Tf2;
    let root_path = "./ex/tf/";
    let vpk = VpkState::new(root_path, game_id).expect("Failed to load VPKs for the game");
    let loaded_textures = LoadedTextures::default();

    let end_time = std::time::Instant::now();
    println!("Loaded VPKs in {:?}", end_time - start_time);

    // use std::io::Write;
    // let root = &vpk.textures.data;
    // let mut out_file = std::fs::File::create("tf2_textures.txt").unwrap();
    // write!(out_file, "{root:#?}").unwrap();

    #[allow(clippy::default_constructed_unit_structs)]
    let mut app = App::new();

    // embedded_asset!(app, "../assets/gizmo_material.wgsl");

    app.insert_resource(Msaa::Sample4)
        // .insert_resource(ClearColor(Color::rgb(0.1, 0.2, 0.3)))
        // .insert_resource(WgpuSettings {
        //     // Wireframe
        //     features: WgpuFeatures::POLYGON_MODE_LINE,
        //     ..Default::default()
        // })
        .insert_resource(vpk)
        .insert_resource(loaded_textures)
        .insert_resource(conf)
        .add_plugins(DefaultPlugins)
        // .add_plugins(WireframePlugin)
        .add_plugins(LookTransformPlugin)
        .add_plugins(UnrealCameraPlugin::default())
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(OutlinePlugin)
        .add_systems(Startup, setup)
        // Not sure if this should be preupdate or not
        .add_systems(PreUpdate, update_visibility)
        .add_systems(Update, leafvis_frame)
        .run();
}

/// The index of a face in the BSP
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct FaceIndex(pub usize);

fn setup(
    mut commands: Commands,
    mut asset_server: ResMut<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmo_conf: ResMut<GizmoConfig>,
    mut images: ResMut<Assets<Image>>,
    mut shaders: ResMut<Assets<Shader>>,
    vpk: Res<VpkState>,
    mut loaded_textures: ResMut<LoadedTextures>,
) {
    println!("Setup");

    loaded_textures.missing_texture = images.add(quell::material::missing_texture());

    gizmo_conf.enabled = true;
    gizmo_conf.depth_bias = -1.;

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
                keyboard_mvmt_sensitivity: 5.0,
                // keyboard_mvmt_sensitivity: 40.0,
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

    let transform = Transform::default();
    // commands.spawn((
    //     MaterialMeshBundle {
    //         mesh: meshes.add(Mesh::from(shape::Cube { size: 10.0 })),
    //         material: info.red_gizmo.clone(),
    //         transform,
    //         ..Default::default()
    //     },
    //     // LeafvisFrame,
    // ));
    // let m = StandardMaterial {
    //     cull_mode: Some(Face)
    //     ..Default::default()
    // };
    // let mut cube_mesh = Mesh::from(shape::Cube { size: 10.0 });
    // // cube_mesh.generate_outline_normals().unwrap();
    // commands.spawn((
    //     MaterialMeshBundle {
    //         mesh: meshes.add(cube_mesh),
    //         material: materials.add(StandardMaterial {
    //             base_color: Color::rgb(0.0, 1.0, 0.0),
    //             unlit: true,
    //             cull_mode: None,
    //             // depth_bias: 1000.0,
    //             ..Default::default()
    //         }),
    //         transform: transform.with_translation(Vec3::new(0.0, 10.0, 0.0)),
    //         ..Default::default()
    //     },
    //     // bevy_mod_outline::OutlineBundle {
    //     //     outline: OutlineVolume {
    //     //         visible: true,
    //     //         colour: Color::rgb(0.0, 0.0, 1.0),
    //     //         width: 10.0,
    //     //     },
    //     //     stencil: OutlineStencil {
    //     //         offset: 5.0,
    //     //         enabled: true,
    //     //     },
    //     //     computed: Default::default(),
    //     //     mode: OutlineMode::default(),
    //     // }, // LeafvisFrame,
    // ));
    // let mut cube_mesh = Mesh::from(Cube { size: 10.0 });
    // cube_mesh.generate_outline_normals().unwrap();
    // commands
    //     .spawn(PbrBundle {
    //         mesh: meshes.add(cube_mesh),
    //         material: materials.add(Color::rgb(0.1, 0.1, 0.9).into()),
    //         transform: Transform::from_xyz(0.0, 15.0, 0.0),
    //         ..default()
    //     })
    //     .insert(OutlineBundle {
    //         outline: OutlineVolume {
    //             visible: true,
    //             colour: Color::rgba(0.0, 1.0, 0.0, 1.0),
    //             width: 25.0,
    //         },
    //         ..default()
    //     });

    // Process the map

    let map_path = "ex/ctf_2fort.bsp";
    // let map_path = "ex/tf/tf/maps/test.bsp";
    let mut map = GameMap::from_path(map_path).unwrap();
    {
        load_materials(&vpk, &mut loaded_textures, &mut images, &map).unwrap();

        let start_time = std::time::Instant::now();

        // TODO: would it be better to do a Task based system likes the async_compute example?

        // TODO: We might be able to do something wacky and maybe more efficient by reserving
        // handles before hand.

        println!("Model count: #{}", map.bsp.models.len());

        let faces = construct_meshes(&loaded_textures, &map).collect::<Vec<_>>();
        let materials = &mut materials;
        let meshes = &mut meshes;
        let cmds = faces
            .into_iter()
            .map(move |face_info| {
                let FaceInfo {
                    mesh,
                    material,
                    transform,
                    face_i,
                } = face_info;
                let mesh = meshes.add(mesh);
                let material = materials.add(material);

                (
                    PbrBundle {
                        mesh,
                        material,
                        transform,
                        ..Default::default()
                    },
                    FaceIndex(face_i),
                )
            })
            // We have to collect a second time because spawn_batch requires a 'static
            // iterator
            .collect::<Vec<_>>();

        // commands.spawn_batch(cmds);
        for (pbr, face_i) in cmds {
            let ent = commands.spawn((pbr, face_i));
            map.faces.insert(face_i.0, ent.id());
        }

        let end_time = std::time::Instant::now();

        println!("Loaded map in {:?}", end_time - start_time);
    }
    // spawn_leaf_boundaries(&mut commands, &map, &mut *meshes, &mut *materials);

    commands.insert_resource(map);
}

// TODO: possibly we should group faces under one parent node so we can hide them all at once?
fn update_visibility(
    // commands: Commands,
    // meshes: Res<Assets<Mesh>>,
    map: Res<GameMap>,
    mut nodes: Query<(&FaceIndex, &mut Visibility, &Transform)>,
    cameras: Query<(&UnrealCameraController, &Transform)>,
    conf: Res<Config>,
) {
    if conf.render.lock_pvs {
        return;
    }

    if conf.render.no_vis {
        // TODO: We should cache that we've already done this somehow, or listen for when it
        // changes and do it once.
        for (_, mut vis, _) in nodes.iter_mut() {
            *vis = Visibility::Visible;
        }
        return;
    }
    // It seems like if we go to the blu spawn then we get in proper clusters, is everything
    // shifted badly somehow?? Or are positions supposed to be relative to some origin?
    // for (_camera, transform) in cameras.iter() {
    //     let pos = transform.translation.to_array();
    //     let pos = unrotate(pos);
    //     let pos = unscale(pos);
    //     let pos = vbsp::Vector {
    //         x: pos[0],
    //         y: pos[1],
    //         z: pos[2],
    //     };

    //     let leaf = map.bsp.leaf_at(pos);
    //     if leaf.cluster != -1 {
    //         println!("Camera: {transform:?} -> {pos:?} -> {:?}", leaf.cluster);
    //     }
    // }

    // The way visibility works in BSP is that each point is in exactly one leaf (which are convex,
    // but whatever).
    // Enterable leaves (visleaves) gets a 'cluster number'.
    // Essentially the cluster number is just an id for areas you can be in, which determines what
    // other areas are visible, thus saving work at runtime.

    // TODO: bsp article mentions that there is only ever one leaf per cluster in old source maps,
    // but some CS:GO maps have multiple leaves in the same cluster, do we support that?

    // FIXME: This code is broken!
    // It works in my very simple test map where everything is seemingly visible from everywhere
    // else, but it does not work in ctf_2fort at all!
    // It seems like it basically always gets a leaf with -1 cluster, which is nothing, so it
    // doesn't show anything.
    // If we zoom out very far then we might get something, but I expect that it is going outside
    // the skybox, and at times it crashed due to the bitvec.set in vbsp being out of bounds.
    // (though I've added a check in that code).
    //
    // I'm unsure what the underlying issue is. I've glanced at alternate implementations and they
    // seem like mine.
    // The parsing code in vbsp seems fine for visdata, and swapping it to reading pvs/pas
    // separately did not help.
    // Rewriting the leaf at function and trying to rewrite the visdata decompression didn't help
    // either.
    //
    // It is possible that I'm getting the position of the camera incorrectly, but I'm not sure how
    // it would be so.

    // TODO: use a smallvec
    let mut visible_sets = Vec::with_capacity(2);
    for (_camera, transform) in cameras.iter() {
        let pos = transform_to_vbsp(*transform);
        // TODO: I don't know if this is the best method to find the leaf?
        let leaf = map.bsp.leaf_at(pos);
        // println!("Camera: {transform:?} -> {pos:?} -> {:?}", leaf.cluster);

        if let Some(vis_set) = leaf.visible_set() {
            visible_sets.push(vis_set);
        }
    }

    // // let zero_leaf = map.bsp.leaf_at(vbsp::Vector {
    // //     x: 0.0,
    // //     y: 0.0,
    // //     z: 0.0,
    // // });
    // // if let Some(vis_set) = zero_leaf.visible_set() {
    // //     visible_sets.push(vis_set);
    // // }
    // // let zero_leaf = &*zero_leaf;
    // // println!("Zero leaf: {zero_leaf:?}");

    // // TODO: will this run change detection immediately, or is bevy smart and only does that if it
    // // actually changed?
    // We first have to set all the visibility to hidden
    for (_, mut vis, _) in nodes.iter_mut() {
        *vis = Visibility::Hidden;
    }

    let mut visible_count = 0;
    let mut face_count = 0;
    let mut skipped_faces = 0;
    for visible_leaf in visible_sets.into_iter().flatten() {
        for (face_i, _face) in visible_leaf.faces_enumerate() {
            face_count += 1;
            // println!("Face i: {face_i}");
            // println!("Faces: {:?}", map.faces);
            let Some(entity) = map.faces.get(&face_i) else {
                // That we don't have an index implies that there's faces we don't create..
                // I at first thought this must be displacements (which would also fit!) but it
                // even happens for my small test map.
                skipped_faces += 1;
                continue;
            };
            if let Ok((_, mut vis, _)) = nodes.get_mut(*entity) {
                *vis = Visibility::Visible;
                visible_count += 1;
            }
        }
    }
    println!(
        "Visible faces: {visible_count}; face count: {face_count}; skipped faces: {skipped_faces}",
    );

    if visible_count == 0 {
        // No visible faces, so they're probably outside the map, so we simply add the entire map
        // This should typically not happen during normal gameplay, and if it does happen remotely
        // often then we should try methods to avoid it.
        // (ex: like if cameras for mirrors end up being considered inside the wall then we should
        // try fixing that, via something smarter)

        for (_, mut vis, _) in nodes.iter_mut() {
            *vis = Visibility::Visible;
        }
    }
}

// TODO: This could be useful if we made it update the color of the leaf boundaries based on the
// distance of the camera. How efficiently can we recreate materials with different colors?
// const LEAF_MIN_ALPHA: f32 = 0.05;
// const LEAF_MAX_ALPHA: f32 = 0.4;

// #[derive(Debug, Clone, Component)]
// pub struct LeafBoundary;

// fn update_leaf_boundaries(leaves: Query<(&LeafBoundary, &mut Handle<StandardMaterial>, camera)>) {

// }

// fn config_change_check(
//     mut commands: Commands,
//     mut mesh: ResMut<Assets<Mesh>>,
//     mut materials: ResMut<Assets<StandardMaterial>>,
//     cameras: Query<(&UnrealCameraController, &Transform)>,
//     map: Res<GameMap>,
//     conf: Res<Config>,
// ) {
//     // If the config hasn't changed, then we don't bother.
//     // Note that this is considered changed right at the start of the program.
//     if !conf.is_changed() {
//         return;
//     }

//     render_config_check(
//         &mut commands,
//         &mut mesh,
//         &mut materials,
//         cameras,
//         &map,
//         &conf,
//     );
// }

// fn render_config_check(
//     commands: &mut Commands,
//     mesh: &mut Assets<Mesh>,
//     materials: &mut Assets<StandardMaterial>,
//     cameras: Query<(&UnrealCameraController, &Transform)>,
//     map: &GameMap,
//     conf: &Config,
// ) {
//     let r = &conf.render;

//     add_leafvis(commands, mesh, materials, cameras, map, r.mat.leafvis);
// }

// TODO(minor): Can we do something where it only shows the leaf boundaries to the relevant camera
// and none of the other cameras? Too expensive?

#[derive(Debug, Clone, Component)]
pub struct LeafvisFrame;

fn leafvis_frame(
    mut commands: Commands,
    mut mesh: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<(&UnrealCameraController, &Transform)>,
    map: Res<GameMap>,
    conf: Res<Config>,
    mut ex_leaves: Query<(Entity, &LeafvisFrame)>,
    mut gizmos: Gizmos,
) {
    // FIXME: only add these if it specifically has changed and they aren't already added!
    // And we want to remove the old ones if we change value
    let leaves = match conf.render.mat.leafvis {
        MatLeafvis::Off => Vec::new(),
        MatLeafvis::CurrentVisleaf => cameras
            .iter()
            .map(|(_camera, transform)| {
                let p = transform_to_vbsp(*transform);
                let leaf = map.bsp.leaf_at(p);
                if leaf.cluster != -1 {
                    // println!("Camera: {transform:?} -> {p:?}; leaf: {leaf:?}");
                }
                leaf
            })
            .filter(|leaf| leaf.cluster != -1)
            .collect::<Vec<_>>(),
        MatLeafvis::CurrentViscluster => todo!(),
        MatLeafvis::AllVisleaves => todo!(),
    };

    // let leaves = {
    //     let p = transform_to_vbsp(Transform::from_xyz(-123.6, 32., 140.));
    //     println!("p: {p:?}");
    //     let leaf = map.bsp.leaf_at(p);
    //     if leaf.cluster != -1 {
    //         // println!("Camera: {p:?}; leaf: {leaf:?}");
    //         vec![leaf]
    //     } else {
    //         vec![]
    //     }
    // };
    // TODO: be smarter about this
    for (ent, _) in ex_leaves.iter_mut() {
        commands.entity(ent).despawn();
    }

    for (camera, transform) in cameras.iter() {
        let tra = transform.translation;
        println!("Camera: {tra:?}");
    }

    println!("Leaf count: {}", leaves.len());
    for leaf in leaves {
        // For each leaf we will use its min/max to create a wireframe box.

        let mins = leaf.mins;
        let maxs = leaf.maxs;

        let mins = [mins[0] as f32, mins[1] as f32, mins[2] as f32];
        let maxs = [maxs[0] as f32, maxs[1] as f32, maxs[2] as f32];

        let mins = rotate(scale(mins));
        let maxs = rotate(scale(maxs));

        let mins: Vec3 = Vec3::from_array(mins);
        let maxs: Vec3 = Vec3::from_array(maxs);
        let color = Color::rgb(1.0, 0.0, 0.0);
        // gizmos.rect(position, rotation, size, color);
        // For some reason it doesn't have a 3d box, so we have to do it manually
        // size is a vec2

        // Calculate the corners of the box
        let front_bottom_left = mins;
        let front_bottom_right = Vec3::new(maxs.x, mins.y, mins.z);
        let front_top_left = Vec3::new(mins.x, maxs.y, mins.z);
        let front_top_right = Vec3::new(maxs.x, maxs.y, mins.z);

        let back_bottom_left = Vec3::new(mins.x, mins.y, maxs.z);
        let back_bottom_right = Vec3::new(maxs.x, mins.y, maxs.z);
        let back_top_left = Vec3::new(mins.x, maxs.y, maxs.z);
        let back_top_right = maxs;

        // Draw the 12 edges of the box
        gizmos.line(front_bottom_left, front_bottom_right, color);
        gizmos.line(front_bottom_right, front_top_right, color);
        gizmos.line(front_top_right, front_top_left, color);
        gizmos.line(front_top_left, front_bottom_left, color);

        gizmos.line(back_bottom_left, back_bottom_right, color);
        gizmos.line(back_bottom_right, back_top_right, color);
        gizmos.line(back_top_right, back_top_left, color);
        gizmos.line(back_top_left, back_bottom_left, color);

        gizmos.line(front_bottom_left, back_bottom_left, color);
        gizmos.line(front_bottom_right, back_bottom_right, color);
        gizmos.line(front_top_left, back_top_left, color);
        gizmos.line(front_top_right, back_top_right, color);
    }

    // Way too noisy, might be more useful if we make it stop rendering ones which are farther away
    // for leaf in map.bsp.leaves.iter() {
    //     let mins = leaf.mins;
    //     let maxs = leaf.maxs;

    //     let mins = [mins[0] as f32, mins[1] as f32, mins[2] as f32];
    //     let maxs = [maxs[0] as f32, maxs[1] as f32, maxs[2] as f32];

    //     let mins = rotate(scale(mins));
    //     let maxs = rotate(scale(maxs));

    //     let mins: Vec3 = Vec3::from_array(mins);
    //     let maxs: Vec3 = Vec3::from_array(maxs);
    //     let color = Color::rgba(0.0, 1.0, 0.0, 0.1);
    //     // gizmos.rect(position, rotation, size, color);
    //     // For some reason it doesn't have a 3d box, so we have to do it manually
    //     // size is a vec2

    //     // Define corners of the box

    //     // Calculate the corners of the box
    //     let front_bottom_left = mins;
    //     let front_bottom_right = Vec3::new(maxs.x, mins.y, mins.z);
    //     let front_top_left = Vec3::new(mins.x, maxs.y, mins.z);
    //     let front_top_right = Vec3::new(maxs.x, maxs.y, mins.z);

    //     let back_bottom_left = Vec3::new(mins.x, mins.y, maxs.z);
    //     let back_bottom_right = Vec3::new(maxs.x, mins.y, maxs.z);
    //     let back_top_left = Vec3::new(mins.x, maxs.y, maxs.z);
    //     let back_top_right = maxs;

    //     // Draw the 12 edges of
    //     gizmos.line(front_bottom_left, front_bottom_right, color);
    //     gizmos.line(front_bottom_right, front_top_right, color);
    //     gizmos.line(front_top_right, front_top_left, color);
    //     gizmos.line(front_top_left, front_bottom_left, color);

    //     gizmos.line(back_bottom_left, back_bottom_right, color);
    //     gizmos.line(back_bottom_right, back_top_right, color);
    //     gizmos.line(back_top_right, back_top_left, color);
    //     gizmos.line(back_top_left, back_bottom_left, color);

    //     gizmos.line(front_bottom_left, back_bottom_left, color);
    //     gizmos.line(front_bottom_right, back_bottom_right, color);
    //     gizmos.line(front_top_left, back_top_left, color);
    //     gizmos.line(front_top_right, back_top_right, color);
    // }
}
