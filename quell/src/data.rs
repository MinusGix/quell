use std::{borrow::Cow, collections::HashMap};

use bevy::{
    prelude::{Assets, Handle, Image, Resource},
    render::render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
};

use crate::map::GameMap;

// TODO: We could preconvert vtf files to efficient formats, and then load those instead

// TODO: on map change you should remove all 'map' textures

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LSrc {
    TexturesVPK,
    MiscVPK,
    Map,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LTexture {
    pub image: Handle<Image>,
    pub src: LSrc,
}

/// Textures that have been loaded, by their lowercase name  
/// These are (typically? always?) from the `materials/` folder
#[derive(Default, Clone, Resource)]
pub struct LoadedTextures(HashMap<String, LTexture>);
impl LoadedTextures {
    /// Find a texture by its lowercase name
    pub fn find(&self, name: &str) -> Option<&LTexture> {
        for (image_name, image) in self.0.iter() {
            if name.eq_ignore_ascii_case(&image_name) {
                return Some(image);
            }
        }

        None
    }

    // TODO: could we somehow make this an asset loader?
    /// Load a VMT file, and load the texture it points to
    pub fn load_texture(
        &mut self,
        vpk: &VpkState,
        map: Option<&GameMap>,
        images: &mut Assets<Image>,
        name: &str,
    ) -> Option<Handle<Image>> {
        if let Some(ltexture) = self.find(name) {
            return Some(ltexture.image.clone());
        }

        let Some((image, src)) = load_texture(vpk, map, name) else {
            // testing panic
            panic!("Could not load texture: {name:?}");
        };
        let (width, height) = image.dimensions();
        let size = Extent3d {
            width: width as u32,
            height: height as u32,
            ..Default::default()
        };
        let image = Image {
            data: image.into_raw(),
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            ..Default::default()
        };

        let handle = images.add(image);

        self.0.insert(
            name.to_lowercase(),
            LTexture {
                image: handle.clone(),
                src,
            },
        );

        Some(handle)
    }
}

#[derive(Resource)]
pub struct VpkState {
    // TODO: should these even be named? Should we just have a general pool of vpks that we look at?
    pub textures: VpkData,
    pub misc: VpkData,
}
impl VpkState {
    pub fn new() -> VpkState {
        let textures = VpkData::load("./ex/tf/tf/tf2_textures_dir.vpk").unwrap();
        let misc = VpkData::load("./ex/tf/tf/tf2_misc_dir.vpk").unwrap();
        // TODO: sound
        VpkState { textures, misc }
    }

    /// Find an entry in the loaded vpks.  
    /// This ignores case.
    pub fn find<'a>(&'a self, name: &str) -> Option<(&'a vpk::entry::VPKEntry, LSrc)> {
        if let Some(entry) = self.textures.find(name) {
            return Some((entry, LSrc::TexturesVPK));
        }

        if let Some(entry) = self.misc.find(name) {
            return Some((entry, LSrc::MiscVPK));
        }

        None
    }

    /// Find a texture entry in the loaded vpks.
    /// This ignores case.
    pub fn find_texture<'a>(&'a self, name: &str) -> Option<(&'a vpk::entry::VPKEntry, LSrc)> {
        if let Some(entry) = self.textures.find_texture(name) {
            return Some((entry, LSrc::TexturesVPK));
        }

        if let Some(entry) = self.misc.find_texture(name) {
            return Some((entry, LSrc::MiscVPK));
        }

        None
    }
}

pub struct VpkData {
    pub data: vpk::VPK,
}
impl VpkData {
    // TODO: use paths
    pub fn load(path: &str) -> Result<VpkData, vpk::Error> {
        let data = vpk::from_path(path)?;
        Ok(VpkData { data })
    }

    /// Find an entry in the loaded vpk.
    /// This ignores case.
    pub fn find<'a>(&'a self, name: &str) -> Option<&'a vpk::entry::VPKEntry> {
        for (file, entry) in self.data.tree.iter() {
            if file.eq_ignore_ascii_case(name) {
                return Some(entry);
            }
        }

        None
    }

    /// Find an entry for a texture, looking in the materials folder
    /// This ignores case.
    pub fn find_texture<'a>(&'a self, name: &str) -> Option<&'a vpk::entry::VPKEntry> {
        for (file, entry) in self.data.tree.iter() {
            if file.starts_with("materials/") && file.ends_with(".vtf") {
                let file = file.trim_start_matches("materials/");
                let file = file.trim_end_matches(".vtf");
                if file.eq_ignore_ascii_case(name) {
                    return Some(entry);
                }
            }
        }

        None
    }
}

fn load_texture(
    vpk: &VpkState,
    map: Option<&GameMap>,
    name: &str,
) -> Option<(image::ImageBuffer<image::Rgba<u8>, Vec<u8>>, LSrc)> {
    // let Some((tex, src)) = vpk.find_texture(name) else {
    //     panic!("Failed to find texture {name:?}");
    // };
    // println!("Loaded texture: {}", name);
    // // TODO: check what possible errors could occur
    // let tex = tex.get().unwrap();
    // let tex = vtf::from_bytes(&tex).unwrap();
    // let image = tex.highres_image.decode(0).unwrap();
    // Some((image.into_rgba8(), src))
    let (tex, src) = find_texture(vpk, map, name)?;
    let tex = vtf::from_bytes(&tex).unwrap();
    let image = tex.highres_image.decode(0).unwrap();
    Some((image.into_rgba8(), src))
}

fn find_texture<'a>(
    vpk: &'a VpkState,
    map: Option<&'a GameMap>,
    name: &str,
) -> Option<(Cow<'a, [u8]>, LSrc)> {
    // TODO: does map take precedence over vpks?
    if let Some((tex, src)) = vpk.find_texture(name) {
        let tex = tex.get().unwrap();
        // let tex = vtf::from_bytes(&tex).unwrap();
        Some((tex, src))
    } else if let Some(map) = map {
        let (tex, src) = map.find_texture(name)?;
        // let tex = vtf::from_bytes(&tex).unwrap();
        Some((Cow::Owned(tex), src))
    } else {
        panic!("Failed to find texture {name:?}");
    }
}
