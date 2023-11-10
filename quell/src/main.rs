use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};

use quell::{
    data::{GameId, LoadedTextures, VpkState},
    map::GameMap,
    material::load_materials,
    mesh::{construct_meshes, FaceInfo},
};

use rayon::prelude::ParallelIterator;
use smooth_bevy_cameras::{
    controllers::unreal::{UnrealCameraBundle, UnrealCameraController, UnrealCameraPlugin},
    LookTransformPlugin,
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
    // let root = &vpk.textures.data;
    // let mut out_file = std::fs::File::create("tf2_textures.txt").unwrap();
    // write!(out_file, "{root:#?}").unwrap();

    #[allow(clippy::default_constructed_unit_structs)]
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
        // Not sure if this should be preupdate or not
        .add_systems(PreUpdate, update_visibility)
        .run();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct BSPHeadNode(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct LeafFaceId(pub u16);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    vpk: Res<VpkState>,
    mut loaded_textures: ResMut<LoadedTextures>,
) {
    println!("Setup");

    loaded_textures.missing_texture = images.add(quell::material::missing_texture());

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

        load_materials(&vpk, &mut loaded_textures, &mut images, &map).unwrap();

        let start_time = std::time::Instant::now();

        // TODO: would it be better to do a Task based system likes the async_compute example?

        // TODO: We might be able to do something wacky and maybe more efficient by reserving
        // handles before hand.

        println!("Model count: #{}", map.bsp.models.len());

        let cmds = construct_meshes(&loaded_textures, &map)
            .collect::<Vec<_>>()
            .into_iter()
            .map(move |face_info| {
                let FaceInfo {
                    mesh,
                    material,
                    transform,
                } = face_info;
                let mesh = meshes.add(mesh);
                let material = materials.add(material);

                (PbrBundle {
                    mesh,
                    material,
                    transform,
                    ..Default::default()
                },)
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

// TODO: possibly we should group faces under one parent node so we can hide them all at once?
fn update_visibility(
    commands: Commands,
    meshes: Res<Assets<Mesh>>,
    map: Res<GameMap>,
    mut nodes: Query<(&BSPHeadNode, &mut Visibility, &Transform)>,
    cameras: Query<(&UnrealCameraController, &Transform)>,
) {
    // The way visibility works in BSP is that each point is in exactly one leaf (which are convex,
    // but whatever).
    // Enterable leaves (visleaves) gets a 'cluster number'.
    // Essentially the cluster number is just an id for areas you can be in, which determines what
    // other areas are visible, thus saving work at runtime.

    // TODO: bsp article mentions that there is only ever one leaf per cluster in old source maps,
    // but some CS:GO maps have multiple leaves in the same cluster, do we support that?

    // // We iterate over all the entities with a BSP head node (currently faces) to get the node that
    // // they are at (since BSP is a tree, the leaves are somewhere below, I believe?)
    // for (head_node, visibility, transform) in head_nodes.iter() {
    //     // let node = map.bsp.node(head_node.0).unwrap();
    //     let pos = transform.translation;
    //     let pos = vbsp::Vector {
    //         x: pos.x,
    //         y: pos.y,
    //         z: pos.z,
    //     };
    //     // TODO: I don't know if this is the best method
    //     let leaf = map.bsp.leaf_at(pos);

    //     // todo
    // }

    // TODO: we can probably compute this more efficiently by just iterating over the visible
    // clusters in the visdata directly, and thus avoid any allocs
    // Though the obvious way of iterating that I can see has the issue that we'd have to set
    // visibility to hidden for all of them, and then undo that, but unless they do something fancy
    // that should be cheap & fine?
    // let mut visible_clusters = BitVec::new();

    // // Compute the visible clusters from the camera locations.
    // // We have to allow separate cameras, because they could be in different locations.
    // for (_camera, transform) in cameras.iter() {
    //     let pos = transform.translation;
    //     let pos = vbsp::Vector {
    //         x: pos.x,
    //         y: pos.y,
    //         z: pos.z,
    //     };
    //     // TODO: I don't know if this is the best method to find the leaf?
    //     let leaf = map.bsp.leaf_at(pos);
    //     let cluster = leaf.cluster;

    //     let mut vis = map.bsp.vis_data.visible_clusters(cluster);
    //     if visible_clusters.is_empty() {
    //         visible_clusters = vis;
    //     } else {
    //         // First we have to ensure their length matches unfortunately
    //         if visible_clusters.len() < vis.len() {
    //             visible_clusters.resize(vis.len(), false);
    //         } else if visible_clusters.len() > vis.len() {
    //             vis.resize(visible_clusters.len(), false);
    //         }
    //         // Combine it with visible clusters
    //         visible_clusters.bit_or_assign(&vis);
    //     }
    // }

    let mut visible_sets = Vec::with_capacity(2);
    for (_camera, transform) in cameras.iter() {
        let pos = transform.translation;
        let pos = vbsp::Vector {
            x: pos.x,
            y: pos.y,
            z: pos.z,
        };
        // TODO: I don't know if this is the best method to find the leaf?
        let leaf = map.bsp.leaf_at(pos);
        let leaf2 = &*leaf;
        println!("Leaf: {:?}", leaf2);
        if let Some(vis_set) = leaf.visible_set() {
            visible_sets.push(vis_set);
        }
    }

    // TODO: will this run change detection immediately, or is bevy smart and only does that if it
    // actually changed?
    // We first have to set all the visibility to hidden
    for (_, mut vis, _) in nodes.iter_mut() {
        *vis = Visibility::Hidden;
    }

    // for visible_leaf in visible_sets.into_iter().flatten() {
    //     todo!()
    // }

    // // Now set any entries that are visible to visible
    // for cluster_index in 0..visible_clusters.len() {
    //     let visible = visible_clusters[cluster_index];
    //     if visible {
    //         let leaves = map.bsp
    //     }
    // }
}
