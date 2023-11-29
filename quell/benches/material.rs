#![feature(test)]

use bevy::{
    pbr::StandardMaterial,
    prelude::{Assets, Image},
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use quell::{
    data::{GameId, LoadedTextures, VpkState},
    map::GameMap,
    material::load_materials,
};

fn bench_load_materials(c: &mut Criterion) {
    let game_id = GameId::Tf2;
    let root_path = "./ex/tf/";
    let vpk = VpkState::new(root_path, game_id).expect("Failed to load vpk state");
    let map_path = "./ex/ctf_2fort.bsp";
    // let map_path = "ex/tf/tf/maps/test.bsp";
    let map = GameMap::from_path(map_path).expect("Failed to load game map");

    c.bench_function("load-materials1", |b| {
        b.iter(|| {
            let mut images: Assets<Image> = Assets::default();
            let mut materials: Assets<StandardMaterial> = Assets::default();
            let mut loaded_textures = LoadedTextures::default();

            let res = load_materials(
                &vpk,
                &mut loaded_textures,
                &mut images,
                &mut materials,
                &map,
            );

            black_box(res).unwrap();

            println!(
                "Materials loaded: {}; Textures loaded: {}",
                loaded_textures.vmt.len(),
                loaded_textures.vtf.len(),
            );
        });
    });
}

criterion_group!(benches, bench_load_materials);
criterion_main!(benches);
