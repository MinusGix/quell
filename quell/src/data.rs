use std::{borrow::Cow, collections::HashMap};

use bevy::{
    prelude::{Assets, Handle, Image, Resource},
    render::render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
};
use vmt::{VMTError, VMT};

use crate::map::GameMap;

// TODO: We could preconvert vtf files to efficient formats, and then load those instead

// TODO: on map change you should remove all 'map' textures

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LSrc {
    /// Fomr `hl2/hl2_textures_dir.vpk`
    HL2Textures,
    /// From `hl2/hl2_misc_dir.vpk`
    HL2Misc,
    /// Main game textures
    TexturesVPK,
    /// Main misc
    MiscVPK,
    Map,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LTexture {
    pub image: Handle<Image>,
    pub image_src: LSrc,
    pub vmt_src: LSrc,
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

        let Some((vmt, vmt_src)) = find_vmt(vpk, map, name) else {
            // testing panic
            panic!("Could not load vmt: {name:?}");
        };
        println!("VMT: {}", std::str::from_utf8(&vmt).unwrap());
        let vmt = VMT::from_bytes(&vmt).unwrap();
        let mut tmp = None;
        // TODO: support resolving more than one level of vmt includes
        let vmt = vmt
            .resolve(|name| -> Result<VMT<'_>, VMTError> {
                let Some((vmt, _vmt_src)) = find_vmt(vpk, map, name) else {
                    // testing panic
                    panic!("Could not load vmt: {name:?}");
                };
                tmp = Some(vmt);
                // println!("VMT: {}", std::str::from_utf8(&vmt).unwrap());
                let vmt = VMT::from_bytes(tmp.as_ref().unwrap())?;
                println!("Applying: {vmt:?}");
                Ok(vmt)
            })
            .unwrap();

        let Some(base_texture) = vmt.base_texture else {
            // testing panic
            panic!("Could not find base texture in vmt: {name:?}; vmt: {vmt:#?}");
        };
        println!("Base texture: {base_texture:?}");

        let Some((image, image_src)) = load_texture(vpk, map, &base_texture) else {
            // testing panic
            panic!("Could not load texture: {base_texture:?}");
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
                image_src,
                vmt_src,
            },
        );

        Some(handle)
    }
}

#[derive(Resource)]
pub struct VpkState {
    pub hl2_textures: VpkData,
    pub hl2_misc: VpkData,
    // TODO: should these even be named? Should we just have a general pool of vpks that we look at?
    pub textures: VpkData,
    pub misc: VpkData,
}
impl VpkState {
    pub fn new() -> VpkState {
        let hl2_textures = VpkData::load("./ex/tf/hl2/hl2_textures_dir.vpk").unwrap();
        let hl2_misc = VpkData::load("./ex/tf/hl2/hl2_misc_dir.vpk").unwrap();
        let textures = VpkData::load("./ex/tf/tf/tf2_textures_dir.vpk").unwrap();
        let misc = VpkData::load("./ex/tf/tf/tf2_misc_dir.vpk").unwrap();
        // TODO: sound
        VpkState {
            hl2_textures,
            hl2_misc,
            textures,
            misc,
        }
    }

    /// Find an entry in the loaded vpks.  
    /// This ignores case.
    pub fn find<'a>(&'a self, name: &str) -> Option<(&'a vpk::entry::VPKEntry, LSrc)> {
        if let Some(entry) = self.hl2_textures.find(name) {
            return Some((entry, LSrc::HL2Textures));
        }

        if let Some(entry) = self.hl2_misc.find(name) {
            return Some((entry, LSrc::HL2Misc));
        }

        if let Some(entry) = self.textures.find(name) {
            return Some((entry, LSrc::TexturesVPK));
        }

        if let Some(entry) = self.misc.find(name) {
            return Some((entry, LSrc::MiscVPK));
        }

        None
    }

    pub fn find_vmt<'a>(&'a self, name: &str) -> Option<(&'a vpk::entry::VPKEntry, LSrc)> {
        let name = name.strip_prefix("materials/").unwrap_or(name);
        let name = name.strip_suffix(".vmt").unwrap_or(name);

        if let Some(entry) = self.hl2_textures.find_vmt(name) {
            return Some((entry, LSrc::HL2Textures));
        }

        if let Some(entry) = self.hl2_misc.find_vmt(name) {
            return Some((entry, LSrc::HL2Misc));
        }

        if let Some(entry) = self.textures.find_vmt(name) {
            return Some((entry, LSrc::TexturesVPK));
        }

        if let Some(entry) = self.misc.find_vmt(name) {
            return Some((entry, LSrc::MiscVPK));
        }

        None
    }

    /// Find a vtf texture entry in the loaded vpks.
    /// This ignores case.
    pub fn find_texture<'a>(&'a self, name: &str) -> Option<(&'a vpk::entry::VPKEntry, LSrc)> {
        let name = name.strip_prefix("materials/").unwrap_or(name);
        let name = name.strip_suffix(".vtf").unwrap_or(name);

        if let Some(entry) = self.hl2_textures.find_texture(name) {
            return Some((entry, LSrc::HL2Textures));
        }

        if let Some(entry) = self.hl2_misc.find_texture(name) {
            return Some((entry, LSrc::HL2Misc));
        }

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

    pub fn find_with_suffix_prefix<'a>(
        &'a self,
        prefix: &str,
        name: &str,
        suffix: &str,
    ) -> Option<&'a vpk::entry::VPKEntry> {
        for (file, entry) in self.data.tree.iter() {
            if file.starts_with(prefix) && file.ends_with(suffix) {
                let file = file.trim_start_matches(prefix);
                let file = file.trim_end_matches(suffix);
                if file.eq_ignore_ascii_case(name) {
                    return Some(entry);
                }
            }
        }

        None
    }

    pub fn find_vmt<'a>(&'a self, name: &str) -> Option<&'a vpk::entry::VPKEntry> {
        self.find_with_suffix_prefix("materials/", name, ".vmt")
    }

    /// Find an entry for a vtf texture, looking in the materials folder
    /// This ignores case.
    pub fn find_texture<'a>(&'a self, name: &str) -> Option<&'a vpk::entry::VPKEntry> {
        self.find_with_suffix_prefix("materials/", name, ".vtf")
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

fn find_vmt<'a>(
    vpk: &'a VpkState,
    map: Option<&'a GameMap>,
    name: &str,
) -> Option<(Cow<'a, [u8]>, LSrc)> {
    // TODO: does map take precedence over vpks?
    if let Some((tex, src)) = vpk.find_vmt(name) {
        let tex = tex.get().unwrap();
        Some((tex, src))
    } else if let Some(map) = map {
        let (tex, src) = map.find_vmt(name)?;
        Some((Cow::Owned(tex), src))
    } else {
        panic!("Failed to find texture {name:?}");
    }
}
