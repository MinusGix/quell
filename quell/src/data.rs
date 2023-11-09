use std::{borrow::Cow, collections::HashMap, hash::Hash, path::Path, sync::Arc};

use bevy::{
    prelude::{Assets, Handle, Image, Resource},
    render::{
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        texture::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    },
};
use indexmap::Equivalent;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use vmt::{ShaderName, VMTError, VMTItem, VMT};
use vpk::{
    access::{DirFile, DirFileBigRefLowercase},
    vpk::{Ext, ProbableKind},
};

use crate::map::GameMap;

// TODO: We could preconvert vtf files to efficient formats, and then load those instead

// TODO: on map change you should remove all 'map' textures

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VPKSrc {
    /// From `hl2/hl2_textures_dir.vpk`
    HL2Textures,
    /// From `hl2/hl2_misc_dir.vpk`
    HL2Misc,
    /// Main game textures
    TexturesVPK,
    /// Main misc
    MiscVPK,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LSrc {
    Vpk(VPKSrc),
    Map,
}
impl From<VPKSrc> for LSrc {
    fn from(src: VPKSrc) -> Self {
        LSrc::Vpk(src)
    }
}

pub type MaterialName = Arc<str>;
pub type TextureName = Arc<str>;

#[derive(Debug, Clone)]
pub enum MaterialError {
    FindFailure(String),

    Frozen,

    VMT(vmt::VMTError),
    Texture(TextureError),
    Io(Arc<std::io::Error>),
}

impl From<TextureError> for MaterialError {
    fn from(err: TextureError) -> Self {
        MaterialError::Texture(err)
    }
}
impl From<std::io::Error> for MaterialError {
    fn from(err: std::io::Error) -> Self {
        MaterialError::Io(Arc::new(err))
    }
}
impl From<vmt::VMTError> for MaterialError {
    fn from(err: vmt::VMTError) -> Self {
        MaterialError::VMT(err)
    }
}
impl std::error::Error for MaterialError {}
impl std::fmt::Display for MaterialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaterialError::FindFailure(name) => write!(f, "Failed to find material: {}", name),
            MaterialError::Frozen => write!(f, "Cannot load more materials"),
            MaterialError::VMT(err) => write!(f, "VMT error: {}", err),
            MaterialError::Texture(err) => write!(f, "Texture error: {}", err),
            MaterialError::Io(err) => write!(f, "IO error: {}", err),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TextureError {
    NotLoaded,
    FindFailure(String),
    Frozen,

    VPK(Arc<vpk::Error>),
    VTF(Arc<vtf::Error>),
    Io(Arc<std::io::Error>),
}
impl From<vpk::Error> for TextureError {
    fn from(err: vpk::Error) -> Self {
        TextureError::VPK(Arc::new(err))
    }
}
impl From<vtf::Error> for TextureError {
    fn from(err: vtf::Error) -> Self {
        TextureError::VTF(Arc::new(err))
    }
}
impl From<std::io::Error> for TextureError {
    fn from(err: std::io::Error) -> Self {
        TextureError::Io(Arc::new(err))
    }
}
impl std::error::Error for TextureError {}
impl std::fmt::Display for TextureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureError::NotLoaded => write!(f, "Texture not loaded"),
            TextureError::FindFailure(name) => write!(f, "Failed to find texture: {}", name),
            TextureError::Frozen => write!(f, "Cannot load more textures"),
            TextureError::VPK(err) => write!(f, "VPK error: {}", err),
            TextureError::VTF(err) => write!(f, "VTF error: {}", err),
            TextureError::Io(err) => write!(f, "IO error: {}", err),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LMaterial {
    /// Name of vtf
    pub image: Result<TextureName, TextureError>,
    pub vmt_src: LSrc,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LImage {
    pub image: Handle<Image>,
    pub src: LSrc,
}

/// Textures that have been loaded, by their lowercase name  
/// These are (typically? always?) from the `materials/` folder
#[derive(Default, Clone, Resource)]
pub struct LoadedTextures {
    pub missing_texture: Handle<Image>,
    pub vmt: HashMap<MaterialName, LMaterial>,
    pub vtf: HashMap<TextureName, LImage>,
    /// Whether it should refuse to load any more materials/textures
    pub frozen: bool,
}
impl LoadedTextures {
    /// Find a material by its lowercase name
    pub fn find_material(&self, name: &str) -> Option<&LMaterial> {
        for (vmt_name, material) in self.vmt.iter() {
            if name.eq_ignore_ascii_case(vmt_name) {
                return Some(material);
            }
        }

        None
    }

    /// Find a texture by its lowercase name
    pub fn find_texture(&self, name: &str) -> Option<&LImage> {
        for (vtf_name, image) in self.vtf.iter() {
            if name.eq_ignore_ascii_case(vtf_name) {
                return Some(image);
            }
        }

        None
    }

    pub fn find_material_texture(&self, name: &str) -> Option<Result<Handle<Image>, TextureError>> {
        let lmaterial = self.find_material(name)?;

        match &lmaterial.image {
            Ok(name) => {
                let ltexture = self.vtf.get(name).unwrap();
                Some(Ok(ltexture.image.clone()))
            }
            Err(err) => Some(Err(err.clone())),
        }
    }

    // TODO: we could save on memory by removing textures that have already been loaded
    // (in a non-context specific texture area, like the main vpks)
    // though that would require a bit of a different storage.
    // We currently store it by the VMT name, rather than the VTF name
    //
    // but naively adding a separate hashmap doing vtf name -> image
    // would have issues in the future, when the VTF might choose things that change how we store
    // the image.
    // Though this could be maybe avoided by some sort of hash, though that isn't unique enough?

    // TODO: could we somehow make this an asset loader?
    /// Load a VMT file, and load the texture it points to
    pub fn load_material<'a>(
        &mut self,
        vpk: &VpkState,
        map: Option<&GameMap>,
        images: &mut Assets<Image>,
        name: &str,
    ) -> Result<Handle<Image>, MaterialError> {
        if let Some(image) = self.find_material_texture(name) {
            return Ok(image?);
        }

        if self.frozen {
            println!("Frozen for {name:?}");
            return Err(MaterialError::Frozen);
        }

        let info = construct_material_info(vpk, map, name)?;
        let name: MaterialName = name.to_lowercase().into();

        self.load_material_with_info(vpk, map, images, name, info)
    }

    fn load_material_with_info(
        &mut self,
        vpk: &VpkState,
        map: Option<&GameMap>,
        images: &mut Assets<Image>,
        name: MaterialName,
        info: LoadingMaterialInfo,
    ) -> Result<Handle<Image>, MaterialError> {
        if self.frozen {
            return Err(MaterialError::Frozen);
        }

        let lmaterial = LMaterial {
            image: Err(TextureError::NotLoaded),
            vmt_src: info.vmt_src,
        };

        self.vmt.insert(name.clone(), lmaterial);

        // TODO: fallback materials?
        // TODO: normal maps
        // TODO: bump maps

        self.load_texture(vpk, map, images, info.base_texture_name.clone())?;

        let handle = self.vtf.get(&info.base_texture_name).unwrap().image.clone();
        self.vmt.get_mut(&name).unwrap().image = Ok(info.base_texture_name.clone());

        Ok(handle)
    }

    /// Typically this should not be used.
    pub fn insert_material(&mut self, name: Arc<str>, material: LMaterial) {
        self.vmt.insert(name, material);
    }

    /// Note: you should typically not directly use this, you should probably be loading the
    /// material that references this texture.
    pub fn load_texture(
        &mut self,
        vpk: &VpkState,
        map: Option<&GameMap>,
        images: &mut Assets<Image>,
        name: TextureName,
    ) -> Result<(), TextureError> {
        if self.frozen {
            return Err(TextureError::Frozen);
        }

        let (image, image_src) = construct_image(vpk, map, &name)?;

        self.insert_texture_of(images, name, image, image_src)?;

        Ok(())
    }

    pub fn insert_texture_of(
        &mut self,
        images: &mut Assets<Image>,
        name: TextureName,
        image: Image,
        image_src: LSrc,
    ) -> Result<TextureName, TextureError> {
        if self.frozen {
            return Err(TextureError::Frozen);
        }

        let handle = images.add(image);

        self.vtf.insert(
            name.clone(),
            LImage {
                image: handle.clone(),
                src: image_src,
            },
        );

        Ok(name)
    }
}

#[derive(Debug, Clone)]
pub struct LoadingMaterialInfo {
    pub vmt_src: LSrc,
    pub base_texture_name: Arc<str>,
}

pub fn construct_material_info(
    vpk: &VpkState,
    map: Option<&GameMap>,
    name: &str,
) -> Result<LoadingMaterialInfo, MaterialError> {
    let (vmt, vmt_src) = find_vmt(vpk, map, name)?;
    let vmt = VMT::from_bytes(&vmt).map_err(MaterialError::VMT)?;
    let mut tmp = None;
    // TODO: support resolving more than one level of vmt includes
    let vmt = vmt
        .resolve(|name| -> Result<VMT<'_>, MaterialError> {
            let (vmt, _vmt_src) = find_vmt(vpk, map, name)?;
            tmp = Some(vmt);
            let vmt = VMT::from_bytes(tmp.as_ref().unwrap()).map_err(MaterialError::VMT)?;
            Ok(vmt)
        })
        .map_err(|x| x.flip(MaterialError::VMT))?;

    let base_texture_name = match &vmt.shader_name {
        vmt::ShaderName::Water => {
            // TODO: water has things like refract texture and the normal map
            if let Some(base_texture) = vmt.base_texture {
                Arc::from(base_texture.to_lowercase())
            } else if let Some(tool_texture) = vmt.other.get(b"%tooltexture") {
                Arc::from(tool_texture)
            } else {
                panic!("Could not find water texture in vmt: {name:?}; vmt: {vmt:#?}");
            }
        }
        _ => {
            let Some(base_texture) = vmt.base_texture else {
                panic!("Could not find base texture in vmt: {name:?}; vmt: {vmt:#?}");
            };
            Arc::from(base_texture.to_lowercase())
        }
    };

    Ok(LoadingMaterialInfo {
        vmt_src,
        base_texture_name,
    })
}

#[derive(Debug, Clone)]
struct MinimalVMT<'a> {
    pub shader_name: ShaderName<'a>,
    pub base_texture: Option<&'a str>,
    pub include: Option<&'a str>,

    pub tool_texture: Option<&'a str>,
}
impl<'a> MinimalVMT<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<MinimalVMT<'a>, VMTError> {
        let mut vmt_iter = vmt::vmt_from_bytes(bytes);

        let mut vmt = MinimalVMT {
            shader_name: ShaderName::LightmappedGeneric,
            base_texture: None,
            include: None,
            tool_texture: None,
        };

        vmt.shader_name = vmt_iter
            .next()
            .transpose()?
            .and_then(VMTItem::into_shader_name)
            .ok_or(VMTError::MissingShaderName)?;

        let mut sub_depth = 0;
        for item in vmt_iter {
            let item = item?;
            match item {
                VMTItem::ShaderName(_) => unreachable!(),
                VMTItem::KeyValue(key, value) => {
                    if sub_depth != 0 {
                        continue;
                    }

                    if key.eq_ignore_ascii_case(b"$basetexture") {
                        vmt.base_texture =
                            Some(std::str::from_utf8(value).map_err(VMTError::from)?);
                    } else if key.eq_ignore_ascii_case(b"include") {
                        vmt.include = Some(std::str::from_utf8(value).map_err(VMTError::from)?);
                    } else if key.eq_ignore_ascii_case(b"%tooltexture") {
                        vmt.tool_texture =
                            Some(std::str::from_utf8(value).map_err(VMTError::from)?);
                    }
                }
                VMTItem::KeySub(_name) => {
                    sub_depth += 1;
                }
                VMTItem::EndSub => {
                    sub_depth -= 1;
                }
                VMTItem::Comment(_) => {}
            }
        }

        Ok(vmt)
    }

    pub fn apply<'b>(self, other: &MinimalVMT<'b>) -> MinimalVMT<'b>
    where
        'a: 'b,
    {
        let mut vmt = MinimalVMT {
            shader_name: self.shader_name,
            base_texture: self.base_texture,
            include: self.include,
            tool_texture: self.tool_texture,
        };

        if let Some(base_texture_name) = other.base_texture {
            vmt.base_texture = Some(base_texture_name);
        }

        // vmt.include = other.include;

        if let Some(tool_texture) = other.tool_texture {
            vmt.tool_texture = Some(tool_texture);
        }

        vmt
    }
}

// TODO: compare the speed of this with construct_material_info
// this should be faster but it might be dominated by reading the vpk out..
pub fn construct_material_info2(
    vpk: &VpkState,
    map: Option<&GameMap>,
    name: &str,
) -> Result<LoadingMaterialInfo, MaterialError> {
    // let start_time = std::time::Instant::now();
    let (vmt, vmt_src) = find_vmt(vpk, map, name)?;
    // let end_time = std::time::Instant::now();
    // println!("Took {:?} to find vmt {name:?}", end_time - start_time);

    let vmt = MinimalVMT::from_bytes(&vmt)?;
    let tmp;
    let vmt = if let Some(include) = vmt.include {
        let (included_vmt, _) = find_vmt(vpk, map, include)?;
        tmp = Some(included_vmt);
        let included_vmt = MinimalVMT::from_bytes(tmp.as_ref().unwrap())?;

        // vmt.apply(&included_vmt)
        included_vmt.apply(&vmt)
    } else {
        vmt
    };

    let base_texture_name = match &vmt.shader_name {
        ShaderName::Water => {
            // TODO: water has things like refract texture and the normal map
            if let Some(base_texture) = vmt.base_texture {
                Arc::from(base_texture.to_lowercase())
            } else if let Some(tool_texture) = vmt.tool_texture {
                Arc::from(tool_texture.to_lowercase())
            } else {
                panic!("Could not find water texture in vmt: {name:?}; vmt: {vmt:#?}");
            }
        }
        _ => {
            if let Some(base_texture) = vmt.base_texture {
                Arc::from(base_texture.to_lowercase())
            } else {
                panic!("Could not find base texture in vmt: {name:?}; vmt: {vmt:#?}");
            }
        }
    };

    Ok(LoadingMaterialInfo {
        vmt_src,
        base_texture_name,
    })
}

pub fn construct_image(
    vpk: &VpkState,
    map: Option<&GameMap>,
    name: &str,
) -> Result<(Image, LSrc), TextureError> {
    let (image, image_src) = load_texture(vpk, map, name)?;

    let (width, height) = image.dimensions();
    let size = Extent3d {
        width,
        height,
        ..Default::default()
    };

    Ok((
        Image {
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
            sampler: ImageSampler::Descriptor(ImageSamplerDescriptor {
                // TODO: we might have to decide this based on usage?
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                address_mode_w: ImageAddressMode::Repeat,
                ..Default::default()
            }),
            ..Default::default()
        },
        image_src,
    ))
}

#[derive(Debug, Clone)]
pub enum GameId {
    Tf2,
    Hl2,
    // TODO: more
    Custom {
        /// Ex: `tf`
        folder: String,
        /// Ex: `tf2`
        prefix: String,
    },
}
impl GameId {
    pub fn folder(&self) -> &str {
        match self {
            GameId::Tf2 => "tf",
            GameId::Hl2 => "hl2",
            GameId::Custom { folder, .. } => folder,
        }
    }

    pub fn prefix(&self) -> &str {
        match self {
            GameId::Tf2 => "tf2",
            GameId::Hl2 => "hl2",
            GameId::Custom { prefix, .. } => prefix,
        }
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
    /// Create a new [`VpkState`] from the path to the game folder.  
    /// Ex: `C:\Program Files (x86)\Steam\steamapps\common\Team Fortress 2\`  
    /// `game_part` should be the name of the game-specific folder data, like `tf`  
    ///   
    /// Impl note: These are loaded in parallel since currently parsing a dir vpk is actually
    /// relatively slow (8ms for hl2_misc_dir) or pretty slow (30ms for tf2_misc_dir)  
    /// This will be bottlenecked by the slowest entry, however.
    pub fn new(root_path: impl AsRef<Path>, game_id: GameId) -> eyre::Result<VpkState> {
        // TODO: for hl2 this would end up loading things multiple times
        let root_path = root_path.as_ref();
        let hl2_path = root_path.join(GameId::Hl2.folder());
        let game_path = root_path.join(game_id.folder());

        let data: &[(&Path, Cow<'_, str>, ProbableKind)] = &[
            (
                &hl2_path,
                Cow::Borrowed("hl2_textures_dir.vpk"),
                ProbableKind::Hl2Textures,
            ),
            (
                &hl2_path,
                Cow::Borrowed("hl2_misc_dir.vpk"),
                ProbableKind::Hl2Misc,
            ),
            (
                &game_path,
                Cow::Owned(format!("{}_textures_dir.vpk", game_id.prefix())),
                ProbableKind::Tf2Textures,
            ),
            (
                &game_path,
                Cow::Owned(format!("{}_misc_dir.vpk", game_id.prefix())),
                ProbableKind::Tf2Misc,
            ),
        ];

        // TODO(minor): We really shouldn't need to collect into a vec here...
        let mut res = data
            .par_iter()
            .map(|(path, filename, kind)| {
                let path = path.join(filename.as_ref());
                VpkData::load(path, *kind)
            })
            .collect::<Result<Vec<_>, vpk::Error>>()?;

        // We deconstruct the vec into our fields, we don't want to clone
        let hl2_textures = res.remove(0);
        let hl2_misc = res.remove(0);
        let textures = res.remove(0);
        let misc = res.remove(0);

        // TODO: sound
        Ok(VpkState {
            hl2_textures,
            hl2_misc,
            textures,
            misc,
        })
    }

    pub fn iter_vpks(&self) -> impl Iterator<Item = (VPKSrc, &VpkData)> {
        [
            (VPKSrc::HL2Textures, &self.hl2_textures),
            (VPKSrc::HL2Misc, &self.hl2_misc),
            (VPKSrc::TexturesVPK, &self.textures),
            (VPKSrc::MiscVPK, &self.misc),
        ]
        .into_iter()
    }

    pub fn src(&self, src: &VPKSrc) -> Option<&VpkData> {
        match src {
            VPKSrc::HL2Textures => Some(&self.hl2_textures),
            VPKSrc::HL2Misc => Some(&self.hl2_misc),
            VPKSrc::TexturesVPK => Some(&self.textures),
            VPKSrc::MiscVPK => Some(&self.misc),
        }
    }

    pub fn archive_path(&self, src: &VPKSrc, archive_index: u16) -> Option<&str> {
        let src = self.src(&src)?;
        src.data.archive_path(archive_index)
    }

    /// Find an entry in the loaded vpks.  
    /// This ignores case.
    pub fn find<'a>(
        &'a self,
        ext: &Ext<'_>,
        dir: &str,
        filename: &str,
    ) -> Option<(vpk::entry::VPKEntryHandle<'a>, VPKSrc)> {
        if let Some(entry) = self.hl2_textures.find(ext, dir, filename) {
            return Some((entry, VPKSrc::HL2Textures));
        }

        if let Some(entry) = self.hl2_misc.find(ext, dir, filename) {
            return Some((entry, VPKSrc::HL2Misc));
        }

        if let Some(entry) = self.textures.find(ext, dir, filename) {
            return Some((entry, VPKSrc::TexturesVPK));
        }

        if let Some(entry) = self.misc.find(ext, dir, filename) {
            return Some((entry, VPKSrc::MiscVPK));
        }

        None
    }

    pub fn find_vmt<'a>(&'a self, name: &str) -> Option<(vpk::entry::VPKEntryHandle<'a>, VPKSrc)> {
        let name = name.strip_prefix("materials/").unwrap_or(name);
        let name = name.strip_suffix(".vmt").unwrap_or(name);

        let re = DirFileBigRefLowercase::new("materials", name);

        if let Some(entry) = self.hl2_textures.find_vmt_direct(re) {
            return Some((entry, VPKSrc::HL2Textures));
        }

        if let Some(entry) = self.hl2_misc.find_vmt_direct(re) {
            return Some((entry, VPKSrc::HL2Misc));
        }

        if let Some(entry) = self.textures.find_vmt_direct(re) {
            return Some((entry, VPKSrc::TexturesVPK));
        }

        if let Some(entry) = self.misc.find_vmt_direct(re) {
            return Some((entry, VPKSrc::MiscVPK));
        }

        None
    }

    /// Find a vtf texture entry in the loaded vpks.
    /// This ignores case.
    pub fn find_texture<'a>(
        &'a self,
        name: &str,
    ) -> Option<(vpk::entry::VPKEntryHandle<'a>, VPKSrc)> {
        let name = name.strip_prefix("materials/").unwrap_or(name);
        let name = name.strip_suffix(".vtf").unwrap_or(name);

        let re = DirFileBigRefLowercase::new("materials", name);

        if let Some(entry) = self.hl2_textures.find_texture_direct(re) {
            return Some((entry, VPKSrc::HL2Textures));
        }

        if let Some(entry) = self.hl2_misc.find_texture_direct(re) {
            return Some((entry, VPKSrc::HL2Misc));
        }

        if let Some(entry) = self.textures.find_texture_direct(re) {
            return Some((entry, VPKSrc::TexturesVPK));
        }

        if let Some(entry) = self.misc.find_texture_direct(re) {
            return Some((entry, VPKSrc::MiscVPK));
        }

        None
    }
}

pub struct VpkData {
    pub data: vpk::VPK,
}
impl VpkData {
    // TODO: use paths
    pub fn load(
        path: impl AsRef<Path>,
        probable_kind: ProbableKind,
    ) -> Result<VpkData, vpk::Error> {
        let data = vpk::from_path(path, probable_kind)?;
        Ok(VpkData { data })
    }

    /// Find an entry in the loaded vpk.
    /// This ignores case.
    pub fn find<'a>(
        &'a self,
        ext: &Ext<'_>,
        dir: &str,
        filename: &str,
    ) -> Option<vpk::entry::VPKEntryHandle<'a>> {
        self.data.get_ignore_case(ext, dir, filename)
    }

    pub fn find_direct<'a, K: Equivalent<DirFile> + Hash>(
        &'a self,
        ext: &Ext<'_>,
        re: K,
    ) -> Option<vpk::entry::VPKEntryHandle<'a>> {
        self.data.get_direct(ext, re)
    }

    pub fn find_vmt_direct<K: Equivalent<DirFile> + Hash>(
        &self,
        re: K,
    ) -> Option<vpk::entry::VPKEntryHandle<'_>> {
        self.find_direct(&Ext::Vmt, re)
    }

    /// Find an entry for a vtf texture, looking in the materials folder
    /// This ignores case.
    pub fn find_texture_direct<K: Equivalent<DirFile> + Hash>(
        &self,
        re: K,
    ) -> Option<vpk::entry::VPKEntryHandle<'_>> {
        self.find_direct(&Ext::Vtf, re)
    }
}

fn load_texture(
    vpk: &VpkState,
    map: Option<&GameMap>,
    name: &str,
) -> Result<(image::ImageBuffer<image::Rgba<u8>, Vec<u8>>, LSrc), TextureError> {
    let (tex, src) = find_texture_data(vpk, map, name)?;
    let tex = vtf::from_bytes(&tex)?;
    let image = tex.highres_image.decode(0)?;
    Ok((image.into_rgba8(), src))
}

fn find_texture_data<'a>(
    vpk: &'a VpkState,
    map: Option<&'a GameMap>,
    name: &str,
) -> Result<(Cow<'a, [u8]>, LSrc), TextureError> {
    // TODO: does map take precedence over vpks?
    if let Some((tex, src)) = vpk.find_texture(name) {
        let tex = tex.get()?;
        Ok((tex, src.into()))
    } else if let Some(map) = map {
        let tex = map
            .get_texture_data(name)
            .ok_or_else(|| TextureError::FindFailure(name.to_string()))?;
        Ok((Cow::Owned(tex), LSrc::Map))
    } else {
        // TODO: don't panic, this is mostly for testing
        panic!("Failed to find texture {name:?}");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileLoc {
    Vpk { src: VPKSrc, archive_index: u16 },
    Map,
}

pub(crate) fn find_texture<'a>(
    vpk: &'a VpkState,
    map: Option<&'a GameMap>,
    name: &str,
) -> Result<FileLoc, TextureError> {
    if let Some((tex, src)) = vpk.find_texture(name) {
        Ok(FileLoc::Vpk {
            src,
            archive_index: tex.archive_index(),
        })
    } else if let Some(map) = map {
        if map.has_texture(name) {
            Ok(FileLoc::Map)
        } else {
            Err(TextureError::FindFailure(name.to_string()))
        }
    } else {
        // TODO: don't panic, this is mostly for testing
        panic!("Failed to find texture {name:?}");
    }
}

fn find_vmt<'a>(
    vpk: &'a VpkState,
    map: Option<&'a GameMap>,
    name: &str,
) -> Result<(Cow<'a, [u8]>, LSrc), MaterialError> {
    // TODO: does map take precedence over vpks?
    if let Some((tex, src)) = vpk.find_vmt(name) {
        let tex = tex.get()?;
        Ok((tex, src.into()))
    } else if let Some(map) = map {
        let (tex, src) = map
            .find_vmt(name)
            .ok_or_else(|| MaterialError::FindFailure(name.to_string()))?;
        Ok((Cow::Owned(tex), src))
    } else {
        Err(MaterialError::FindFailure(name.to_string()))
    }
}
