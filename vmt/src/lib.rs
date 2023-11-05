use std::{borrow::Cow, collections::HashMap};

#[derive(Debug, Clone)]
pub enum VMTError {
    NoStringStart,
    NoStringEnd,

    Expected(char),

    InvalidBlendMode(u8),

    Utf8Parse(std::str::Utf8Error),
    FloatParse(std::num::ParseFloatError),
    IntParse(std::num::ParseIntError),
    BoolParse(std::str::ParseBoolError),
}
impl From<std::str::Utf8Error> for VMTError {
    fn from(e: std::str::Utf8Error) -> VMTError {
        VMTError::Utf8Parse(e)
    }
}
impl From<std::num::ParseFloatError> for VMTError {
    fn from(e: std::num::ParseFloatError) -> VMTError {
        VMTError::FloatParse(e)
    }
}
impl From<std::str::ParseBoolError> for VMTError {
    fn from(e: std::str::ParseBoolError) -> VMTError {
        VMTError::BoolParse(e)
    }
}
impl From<std::num::ParseIntError> for VMTError {
    fn from(e: std::num::ParseIntError) -> VMTError {
        VMTError::IntParse(e)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShaderName<'a> {
    String(Cow<'a, str>),
    LightmappedGeneric,
    UnlitGeneric,
    VertexLitGeneric,
    // ?
}
impl<'a> From<&'a str> for ShaderName<'a> {
    fn from(s: &str) -> ShaderName {
        match s {
            "LightmappedGeneric" => ShaderName::LightmappedGeneric,
            "UnlitGeneric" => ShaderName::UnlitGeneric,
            "VertexLitGeneric" => ShaderName::VertexLitGeneric,
            _ => ShaderName::String(Cow::Borrowed(s)),
        }
    }
}

/// https://developer.valvesoftware.com/wiki/$detail#.24detailblendfactor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DetailBlendMode {
    DecalModulate = 0,
    /// The color of the detail texture is added to the base texture
    Additive = 1,
    /// The detail texture is applied as a translucent overlay on top of the base texture
    TranslucentDetail = 2,
    /// The detail texture is applied as a translucent overlay, but ignoring its alpha channel.
    /// Instead the blend factor is used to determine how much of the base texture shows through
    /// underneath.
    BlendActorFade = 3,
    /// This effectively flips the normal layering of the two textures.
    /// The detail texture appears 'below', with the base alpha channel controlling it as a
    /// translucent overlay.
    /// The detail alpha channel controls the overall material alpha.
    TranslucentBase = 4,
    /// The color of the detail texture is added to the base texture identically to mode 1, but
    /// this color is unaffected by lighting and therefore appears to glow.
    UnlitAdditive = 5,
    /// This adds color unaffected by lighting like 'Unlit Additive', but first modifies the color
    /// added in two modes, depending on if the blend factor is above/below 0.5.
    UnlitAdditiveThresholdFade = 6,
    /// Only the red and alpha channels of the detail texture are used
    TwoPatternDecalModulate = 7,
    /// The color of the base channel is multiplied by that of the detail texture
    Multiply = 8,
    /// Only the detail alpha channel is used.
    /// It is multiplied with the base texture's alpha channel to produce the final alpha value.
    BaseMaskDetailAlpha = 9,
    /// The detail texture is used as a possibly additional `$ssbump` bumpmap.
    /// The blend factor is ignored.
    SelfShadowedBumpmap = 10,
    /// Utilizes a SSBump texture like an Ambient Occlusion Texture.  
    /// This is done by calculating the above-average 'Luminance' of the SSBUMp
    SelfShadowedBumpmapAlbedo = 11,
}
impl TryFrom<u8> for DetailBlendMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(DetailBlendMode::DecalModulate),
            1 => Ok(DetailBlendMode::Additive),
            2 => Ok(DetailBlendMode::TranslucentDetail),
            3 => Ok(DetailBlendMode::BlendActorFade),
            4 => Ok(DetailBlendMode::TranslucentBase),
            5 => Ok(DetailBlendMode::UnlitAdditive),
            6 => Ok(DetailBlendMode::UnlitAdditiveThresholdFade),
            7 => Ok(DetailBlendMode::TwoPatternDecalModulate),
            8 => Ok(DetailBlendMode::Multiply),
            9 => Ok(DetailBlendMode::BaseMaskDetailAlpha),
            10 => Ok(DetailBlendMode::SelfShadowedBumpmap),
            11 => Ok(DetailBlendMode::SelfShadowedBumpmapAlbedo),
            _ => Err(()),
        }
    }
}

pub type TextureStr<'a> = Cow<'a, str>;
pub type RGB = [f32; 3];

#[derive(Debug, Clone, PartialEq)]
pub struct VMT<'a> {
    pub shader_name: ShaderName<'a>,

    // TODO: some parameters might only be supported with certain shaders?
    /// Defines the albedo texture
    pub base_texture: Option<TextureStr<'a>>,
    /// Whether this material is a decal
    pub decal: Option<bool>,
    /// Links the surface to a set of physical properties
    pub surface_prop: Option<Cow<'a, str>>,
    pub detail: VMTDetail<'a>,
    pub detail2: VMTDetail2<'a>,
    pub base_texture_transform: Option<[f32; 2]>,
    pub color: Option<RGB>,

    // TODO: detail texture transform
    pub phong: Option<f32>,
    pub phong_boost: Option<f32>,
    pub phong_exponent: Option<f32>,
    pub phong_fresnel_ranges: Option<[f32; 3]>,

    pub lightwarp_texture: Option<TextureStr<'a>>,

    pub keywords: Vec<Cow<'a, str>>,
    // TODO: is this some sort of enum?
    pub other: HashMap<Cow<'a, str>, &'a str>,
}
impl<'a> VMT<'a> {
    pub fn from_bytes(b: &'a [u8]) -> Result<VMT<'a>, VMTError> {
        let (b, shader_name) = take_text(b)?;
        let shader_name = ShaderName::from(shader_name);

        let b = take_whitespace(b)?;

        let b = expect_char(b, b'{')?;

        let b = take_whitespace(b)?;

        let mut vmt = VMT::default();
        vmt.shader_name = shader_name;

        let (b, key_name) = take_text(b)?;

        let b = take_whitespace(b)?;

        // TODO: are they all string quoted?
        let (b, val) = take_text(b)?;

        let b = take_whitespace(b)?;

        // TODO: add checks for duplicate key values?
        match key_name {
            "$basetexture" => vmt.base_texture = Some(Cow::Borrowed(val)),
            // TODO: is it space separated or?
            "%keywords" => todo!(),
            "$detail" => vmt.detail.texture = Some(Cow::Borrowed(val)),
            "$detailscale" => vmt.detail.scale = Some(val.parse()?),
            "$detailblendmode" => {
                let val: u8 = val.parse()?;
                let val =
                    DetailBlendMode::try_from(val).map_err(|_| VMTError::InvalidBlendMode(val))?;
                vmt.detail.blend_mode = Some(val);
            }
            "$detailblendfactor" => vmt.detail.blend_factor = Some(val.parse()?),
            "$surfaceprop" => vmt.surface_prop = Some(Cow::Borrowed(val)),
            "$decal" => vmt.decal = Some(val.parse()?),
            "$basetexturetransform" => {
                let (_, val) = take_vec2(val.as_bytes())?;
                vmt.base_texture_transform = Some(val);
            }
            "$color" => {
                let (_, val) = take_vec3(val.as_bytes())?;
                vmt.color = Some(val);
            }
            "$detailtint" => {
                let (_, val) = take_vec3(val.as_bytes())?;
                vmt.detail.tint = Some(val);
            }
            "$detailframe" => vmt.detail.frame = Some(val.parse()?),
            "$detailalphamaskbasetexture" => {
                vmt.detail.alpha_mask_base_texture = Some(val.parse()?)
            }
            "$detail2" => vmt.detail2.texture = Some(Cow::Borrowed(val)),
            "$detailscale2" => vmt.detail2.scale = Some(val.parse()?),
            "$detailblendfactor2" => vmt.detail2.blend_factor = Some(val.parse()?),
            "$detailframe2" => vmt.detail2.frame = Some(val.parse()?),
            "$detailtint2" => {
                let (_, val) = take_vec3(val.as_bytes())?;
                vmt.detail2.tint = Some(val);
            }
            "$phong" => vmt.phong = Some(val.parse()?),
            "$phongboost" => vmt.phong_boost = Some(val.parse()?),
            "$phongexponent" => vmt.phong_exponent = Some(val.parse()?),
            "$phongfresnelranges" => {
                let (_, val) = take_vec3(val.as_bytes())?;
                vmt.phong_fresnel_ranges = Some(val);
            }
            "$lightwarptexture" => vmt.lightwarp_texture = Some(Cow::Borrowed(val)),
            _ => {
                vmt.other.insert(Cow::Borrowed(key_name), val);
            }
        }

        let _b = expect_char(b, b'}')?;

        // typically should be empty by now.
        // let _b = take_whitespace(b)?;

        Ok(vmt)
    }
}
impl<'a> Default for VMT<'a> {
    fn default() -> VMT<'a> {
        VMT {
            shader_name: ShaderName::LightmappedGeneric,
            base_texture: None,
            keywords: Vec::new(),
            detail: VMTDetail::default(),
            detail2: VMTDetail2::default(),
            surface_prop: None,
            decal: None,
            base_texture_transform: None,
            color: None,
            phong: None,
            phong_boost: None,
            phong_exponent: None,
            phong_fresnel_ranges: None,
            lightwarp_texture: None,
            other: HashMap::new(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct VMTDetail<'a> {
    /// `$detail`
    pub texture: Option<TextureStr<'a>>,
    pub tint: Option<RGB>,
    pub frame: Option<u32>,
    pub scale: Option<f32>,
    pub alpha_mask_base_texture: Option<bool>,
    pub blend_mode: Option<DetailBlendMode>,
    pub blend_factor: Option<f32>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct VMTDetail2<'a> {
    /// `$detail2`
    pub texture: Option<TextureStr<'a>>,
    pub scale: Option<f32>,
    pub blend_factor: Option<f32>,
    pub frame: Option<u32>,
    pub tint: Option<RGB>,
}

fn expect_char(bytes: &[u8], c: u8) -> Result<&[u8], VMTError> {
    if bytes.is_empty() {
        return Err(VMTError::Expected(c as char));
    }

    if bytes[0] != c {
        return Err(VMTError::Expected(c as char));
    }

    Ok(&bytes[1..])
}

fn take_whitespace(bytes: &[u8]) -> Result<&[u8], VMTError> {
    let end = bytes
        .iter()
        .position(|&b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());

    Ok(&bytes[end..])
}

/// Parse a single non-whitespaced separated word
/// or a quoted string
fn take_text(bytes: &[u8]) -> Result<(&[u8], &str), VMTError> {
    if bytes.starts_with(b"\"") {
        return take_str(bytes);
    }

    let end = bytes
        .iter()
        .position(|&b| b.is_ascii_whitespace())
        .unwrap_or(bytes.len());

    let (name, bytes) = bytes.split_at(end);

    let name = std::str::from_utf8(name)?;

    Ok((bytes, name))
}

/// Parse a string like `"LightmappedGeneric"`
fn take_str(bytes: &[u8]) -> Result<(&[u8], &str), VMTError> {
    if !bytes.starts_with(b"\"") {
        return Err(VMTError::NoStringStart);
    }

    let bytes = &bytes[1..];

    let end = bytes
        .iter()
        .position(|&b| b == b'"')
        .ok_or(VMTError::NoStringEnd)?;

    let (name, bytes) = bytes.split_at(end);

    let name = std::str::from_utf8(name)?;

    Ok((&bytes[1..], name))
}

fn take_vec2(bytes: &[u8]) -> Result<(&[u8], [f32; 2]), VMTError> {
    let b = expect_char(bytes, b'[')?;
    let b = take_whitespace(b)?;
    let (b, x) = take_text(b)?;
    let b = take_whitespace(b)?;
    let (b, y) = take_text(b)?;
    let b = take_whitespace(b)?;
    let b = expect_char(b, b']')?;

    let x = x.parse()?;
    let y = y.parse()?;

    Ok((b, [x, y]))
}

/// Parse text like `[ 0.4 0.3 0.2 ]`
fn take_vec3(bytes: &[u8]) -> Result<(&[u8], [f32; 3]), VMTError> {
    let b = expect_char(bytes, b'[')?;
    let b = take_whitespace(b)?;
    let (b, x) = take_text(b)?;
    let b = take_whitespace(b)?;
    let (b, y) = take_text(b)?;
    let b = take_whitespace(b)?;
    let (b, z) = take_text(b)?;
    let b = take_whitespace(b)?;
    let b = expect_char(b, b']')?;

    let x = x.parse()?;
    let y = y.parse()?;
    let z = z.parse()?;

    Ok((b, [x, y, z]))
}

#[cfg(test)]
mod test {
    use crate::take_text;

    use super::take_str;

    #[test]
    fn test_take_str() {
        let bytes = b"\"LightmappedGeneric\"";
        let (bytes, name) = take_str(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, "LightmappedGeneric");

        let bytes = b"\"LightmappedGeneric\" \"VertexLitGeneric\"";
        let (bytes, name) = take_str(bytes).unwrap();
        assert_eq!(bytes, b" \"VertexLitGeneric\"");
        assert_eq!(name, "LightmappedGeneric");
        let bytes = &bytes[1..];
        let (bytes, name) = take_str(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, "VertexLitGeneric");
    }

    #[test]
    fn test_take_text() {
        let bytes = b"LightmappedGeneric";
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, "LightmappedGeneric");

        let bytes = b"LightmappedGeneric VertexLitGeneric";
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b" VertexLitGeneric");
        assert_eq!(name, "LightmappedGeneric");
        let bytes = &bytes[1..];
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, "VertexLitGeneric");

        let bytes = b"\"LightmappedGeneric\"";
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, "LightmappedGeneric");

        let bytes = b"\"LightmappedGeneric\" \"VertexLitGeneric\"";
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b" \"VertexLitGeneric\"");
        assert_eq!(name, "LightmappedGeneric");
        let bytes = &bytes[1..];
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, "VertexLitGeneric");
    }
}
