use std::{
    cmp::Ordering,
    collections::HashSet,
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

use bevy::{
    asset::Handle,
    pbr::StandardMaterial,
    prelude::{Assets, Image},
    render::{
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        texture::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    },
};
use dashmap::DashSet;
use rayon::{
    prelude::{IntoParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};

use crate::{
    data::{
        construct_image, construct_material_info2, find_texture, FileLoc, LMaterial,
        LoadedTextures, VpkState,
    },
    map::GameMap,
    util::SeriesCalc,
};

// TODO(minor): Would this work better as an iterator? It could depend on the map's data, so we
// don't have to use handles, because we don't need mutable access.
/// Get all of the names of the materials (vmts) that are referenced in the map.  
/// These names are deduplicated.
pub fn material_names(map: &GameMap) -> Vec<Arc<str>> {
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

            if texture_info.flags.contains(vbsp::TextureFlags::NODRAW)
                || texture_info.flags.contains(vbsp::TextureFlags::SKY)
            {
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

/// Load all the (materials -> textures) in parallel
pub fn load_materials(
    vpk: &VpkState,
    loaded_textures: &mut LoadedTextures,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    map: &GameMap,
) -> eyre::Result<()> {
    let material_names = material_names(map);

    let start_time = std::time::Instant::now();

    let duplicate_counts = AtomicUsize::new(0);

    // The loaded/loading textures
    let l: DashSet<Arc<str>> = DashSet::with_capacity(material_names.len());

    // // Load all the files we'll need to use
    // // But we don't do anything with them, because we are trying to rely on the OS being smart
    // // about caching
    // let files = vpk
    //     .iter_vpks()
    //     .flat_map(|(_, vpk)| vpk.data.archive_paths.iter().map(File::open))
    //     .collect::<Vec<_>>();

    // Iterate over all the materials, loading them.
    // Typically, none of the materials will error at all so the size is usually the same as
    // `material_names`

    // let material_m = Arc::new(Mutex::new(MeanCalc::new()));
    // let image_m = Arc::new(Mutex::new(MeanCalc::new()));
    let material_m = Arc::new(Mutex::new(SeriesCalc::new()));
    let image_m = Arc::new(Mutex::new(SeriesCalc::new()));

    let m_mean = material_m.clone();
    let img_mean = image_m.clone();
    let iter = material_names
        .into_par_iter()
        .filter_map(move |material_name| {
            let start_time = std::time::Instant::now();
            let res = match construct_material_info2(vpk, Some(map), &material_name) {
                Ok(info) => Some((material_name, info)),
                Err(err) => {
                    eprintln!(
                        "Failed to construct material info for {}: {:?}",
                        material_name, err
                    );
                    None
                }
            };
            let end_time = std::time::Instant::now();

            let mut mean = m_mean.lock().unwrap();
            mean.update_dur(end_time - start_time);

            res
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

            let start_time = std::time::Instant::now();
            let res = construct_image(vpk, Some(map), &info.base_texture_name);
            let res = match res {
                Ok((image, img_src)) => Some((material_name, info, Some((image, img_src)))),
                Err(err) => {
                    eprintln!(
                        "Failed to construct image for material {}, texture {}: {:?}",
                        material_name, info.base_texture_name, err
                    );
                    None
                }
            };
            let end_time = std::time::Instant::now();

            let mut mean = img_mean.lock().unwrap();
            mean.update_dur(end_time - start_time);

            res
        })
        .collect::<Vec<_>>();

    println!("L size: #{}", l.len());

    let mut materials_to_load = Vec::with_capacity(iter.len());
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
            image: Ok(info.base_texture_name.clone()),
            mat: Handle::default(),
            vmt_src: info.vmt_src,
        };

        loaded_textures.insert_material(material_name.clone(), material);

        materials_to_load.push((material_name, info));
    }

    for (material_name, info) in materials_to_load {
        // let image = loaded_textures
        //     .find_material_texture(&info.base_texture_name)
        //     .unwrap_or_else(|| {
        //         panic!("Failed to find {:?}", info.base_texture_name)
        //     })?;
        let image = loaded_textures
            .vtf
            .get(&info.base_texture_name)
            .unwrap()
            .image
            .clone();

        let material = make_material(image);
        let material = materials.add(material);

        loaded_textures
            .find_material_mut(&material_name)
            .unwrap()
            .mat = material;
    }

    println!(
        "V: vmt #{}; vtf #{}",
        loaded_textures.vmt.len(),
        loaded_textures.vtf.len()
    );

    let end_time = std::time::Instant::now();

    println!("Loaded textures in {:?}", end_time - start_time,);
    let material_m = material_m.lock().unwrap();
    let image_m = image_m.lock().unwrap();
    println!("Material mean: {:?}", material_m.mean() / 1000.0);
    println!("Image mean: {:?}", image_m.mean() / 1000.0);

    // vec_to_csv(&material_m.entries, "./material.csv").unwrap();
    // vec_to_csv(&image_m.entries, "./image.csv").unwrap();

    println!(
        "Duplicates: {}",
        duplicate_counts.load(std::sync::atomic::Ordering::SeqCst)
    );

    // Stop new textures from being loaded.
    // This is primarily for testing to ensure we don't skip anything.
    loaded_textures.frozen = true;

    Ok(())
}

pub fn make_material(image: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        // base_color: color,
        base_color_texture: Some(image.clone()),
        // alpha_mode: AlphaMode::Blend,
        // unlit: true,
        // emissive_texture: texture,
        // TODO: determine this properly
        // alpha_mode: if color.a() < 1.0 {
        //     AlphaMode::Blend
        // } else {
        //     AlphaMode::Opaque
        // },

        // TODO: might be needed since source uses DX
        // flip_normal_map_y

        // unlit: true,
        // metallic: 0.0,
        // emissive: color,
        // unlit: true,
        // reflectance: 0.0,
        ..Default::default()
    }
}

// fn load_materials2(
//     vpk: &VpkState,
//     loaded_textures: &mut LoadedTextures,
//     images: &mut Assets<Image>,
//     map: &GameMap,
// ) -> eyre::Result<()> {
//     let material_names = material_names(map);

//     let start_time = std::time::Instant::now();

//     // The loaded/loading textures
//     let l: DashSet<Arc<str>> = DashSet::with_capacity(material_names.len());

//     // Our first stages finds all of the VMTs and loads them.
//     // Currently this assumes that the VMTs are cheap to load, which is
//     // probably usually/always true because they'll be in the dir VPK's preload
//     // but I have not actually checked.
//     // TODO: check how many materials are actually in storage files, the average time in my
//     // previous tests makes me think at least some of them are. If a notable amount are, then
//     // we can swap to getting them in the order we need to load them.

//     let material_m = Arc::new(Mutex::new(SeriesCalc::new()));
//     let image_m = Arc::new(Mutex::new(SeriesCalc::new()));

//     let m_mean = material_m.clone();
//     let img_mean = image_m.clone();

//     let iter = material_names
//         .into_par_iter()
//         .filter_map(move |material_name| {
//             let start_time = std::time::Instant::now();
//             let res = match construct_material_info2(vpk, Some(map), &material_name) {
//                 Ok(info) => Some((material_name, info)),
//                 Err(err) => {
//                     eprintln!(
//                         "Failed to construct material info for {}: {:?}",
//                         material_name, err
//                     );
//                     None
//                 }
//             };

//             let end_time = std::time::Instant::now();

//             let mut mean = m_mean.lock().unwrap();
//             mean.update_dur(end_time - start_time);

//             res
//         })
//         // Deduplicate any textures. We still need to add all the different materials, but if they
//         // reference the same texture then we only want to load it once
//         .map(|(material_name, info)| {
//             // TODO: we'll need to extend this when we're loading more texture info from files
//             if l.contains(&info.base_texture_name) {
//                 return (material_name, info, false);
//             }

//             l.insert(info.base_texture_name.clone());

//             // (material_name, info, should_load_img)
//             (material_name, info, true)
//         })
//         .filter_map(|(material_name, info, should_load_img)| {
//             let img_loc = if should_load_img {
//                 Some(find_texture(vpk, Some(map), &info.base_texture_name))
//             } else {
//                 None
//             };

//             Some((material_name, info, img_loc))
//         })
//         .collect::<Vec<_>>();

//     // We've collected the materials and dedup'd the texture references
//     // Now we want to add all the materials that don't need to
//     let mut texture_loc = iter
//         .into_iter()
//         // Add each material to the definition
//         .filter_map(|(material_name, info, img_loc)| {
//             let material = LMaterial {
//                 image: Ok(info.base_texture_name.clone()),
//                 vmt_src: info.vmt_src,
//             };

//             loaded_textures.insert_material(material_name.clone(), material);

//             match img_loc {
//                 Some(Ok(loc)) => Some((info, loc)),
//                 Some(Err(err)) => {
//                     eprintln!(
//                         "Failed to find texture for material {}: {:?}",
//                         material_name, err
//                     );
//                     None
//                 }
//                 None => None,
//             }
//         })
//         .collect::<Vec<_>>();

//     // Now our iter is purely of the textures we need to load
//     // Most textures will not be in the preload, but rather will be in one of the many individual
//     // vpk storage files (suffixed by 000, 001, etc.)
//     // We want to load all of these in parallel, but also do so efficiently.
//     //
//     // We can't simply just open the `File`s and pass them in, because the position is managed by
//     // the `File`, and so that would just completely break in a multithreaded environment.
//     //
//     // It is also dispreferred to open/close the files separately. It might be fine, but it might
//     // also be slower due to constantly talking to the OS.
//     // This is especially a problem because the default order of the textures we're loading
//     // will naturally jump around randomly between the different storage files!
//     //
//     // So what we do here is sort the textures by their storage file.
//     // TODO(minor): We could sort them by their offset in the archive too
//     texture_loc.par_sort_unstable_by(|(_, a), (_, b)| match (a, b) {
//         // We sort by src and then by the archive index within that src
//         (
//             FileLoc::Vpk {
//                 src: a,
//                 archive_index: a_idx,
//             },
//             FileLoc::Vpk {
//                 src: b,
//                 archive_index: b_idx,
//             },
//         ) => match a.cmp(b) {
//             Ordering::Equal => a_idx.cmp(b_idx),
//             other => other,
//         },
//         // TODO(minor): might it be better to put maps in between two vpk loads, so that
//         // there is more time where the threads aren't touching the filesystem?
//         // VPKs are always before maps
//         (FileLoc::Vpk { .. }, FileLoc::Map) => Ordering::Less,
//         (FileLoc::Map, FileLoc::Vpk { .. }) => Ordering::Greater,
//         // TODO(minor): there's probably some ordering we could do for loading from the map's
//         // packfile but it almost certainly doesn't matter much.
//         (FileLoc::Map, FileLoc::Map) => Ordering::Equal,
//     });

//     // Now we can load the textures in parallel
//     // I'm currently breaking it into pieces based on the file type, but that seems less than ideal.

//     // TODO: don't assume there's at least one texture
//     let mut cur_type: FileLoc = texture_loc[0].1.clone();
//     let mut cur_start = 0;
//     let mut work = Vec::new();
//     for (i, (_info, loc)) in texture_loc.iter().enumerate() {
//         if loc != &cur_type {
//             let end = i;
//             work.push((cur_type, cur_start..end));
//             cur_type = loc.clone();
//             cur_start = end;
//         }
//     }
//     if cur_start != texture_loc.len() {
//         work.push((cur_type, cur_start..texture_loc.len()));
//     }

//     let res = work
//         .into_par_iter()
//         .filter_map(|(kind, range)| {
//             let data = &texture_loc[range];

//             let reader = match kind {
//                 FileLoc::Vpk { src, archive_index } => {
//                     let path = vpk.archive_path(&src, archive_index).unwrap();
//                     let Ok(file) = std::fs::File::open(path) else {
//                         eprintln!("Failed to open file: {:?}", path);
//                         return None;
//                     };
//                     Some(file)
//                 }
//                 FileLoc::Map => None,
//             };

//             let mut images = Vec::new();
//             for (info, loc) in data {
//                 assert_eq!(loc, &kind);

//                 let start_time = std::time::Instant::now();
//                 let res = construct_image(vpk, Some(map), &info.base_texture_name);
//                 let res = match res {
//                     Ok((image, img_src)) => Some((info, (image, img_src))),
//                     Err(err) => {
//                         eprintln!(
//                             "Failed to construct image for material, texture {}: {:?}",
//                             info.base_texture_name, err
//                         );
//                         None
//                     }
//                 };
//                 let end_time = std::time::Instant::now();

//                 let mut mean = image_m.lock().unwrap();
//                 mean.update_dur(end_time - start_time);

//                 if let Some(res) = res {
//                     images.push(res);
//                 }
//             }

//             Some(images)
//         })
//         .flat_map(|x| x)
//         .collect::<Vec<_>>();

//     for (info, (image, img_src)) in res {
//         loaded_textures.insert_texture_of(
//             images,
//             info.base_texture_name.clone(),
//             image,
//             img_src,
//         )?;
//     }

//     println!(
//         "V: vmt #{}; vtf #{}",
//         loaded_textures.vmt.len(),
//         loaded_textures.vtf.len()
//     );

//     let end_time = std::time::Instant::now();

//     println!("Loaded textures in {:?};", end_time - start_time);
//     let material_m = material_m.lock().unwrap();
//     let image_m = image_m.lock().unwrap();
//     println!("Material mean: {:?}", material_m.mean() / 1000.0);
//     println!("Image mean: {:?}", image_m.mean() / 1000.0);

//     loaded_textures.frozen = true;

//     Ok(())
// }

pub fn missing_texture() -> Image {
    // Pink and black checkerboard
    #[rustfmt::skip]
    let data = vec![
        255, 0, 255, 255, /**/   0, 0, 0, 255, 
        0, 0, 0, 255,     /**/ 255, 0, 255, 255,
    ];
    Image {
        data,
        texture_descriptor: TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 2,
                height: 2,
                ..Default::default()
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            // TODO: no clue what I should use here
            usage: TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        sampler: ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            address_mode_w: ImageAddressMode::Repeat,
            ..Default::default()
        }),
        ..Default::default()
    }
}
