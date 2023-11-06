use std::{borrow::Cow, collections::HashMap};

use util::{apply, StopOnErr};

use crate::{
    parse::{expect_char, take_text, take_vec3, take_whitespace},
    util::to_lowercase_cow,
};

mod parse;
mod util;

#[derive(Debug, Clone)]
pub enum VMTError<E = ()> {
    MissingShaderName,

    NoStringStart,
    NoStringEnd,

    Expected(char),
    UnexpectedEof,

    InvalidBlendMode(u8),

    Utf8Parse(std::str::Utf8Error),
    FloatParse(std::num::ParseFloatError),
    IntParse(std::num::ParseIntError),
    BoolParse(std::str::ParseBoolError),

    Other(E),
}
impl<E> VMTError<E> {
    pub fn flip(self, f: impl Fn(VMTError) -> E) -> E {
        match self {
            VMTError::MissingShaderName => f(VMTError::MissingShaderName),
            VMTError::NoStringStart => f(VMTError::NoStringStart),
            VMTError::NoStringEnd => f(VMTError::NoStringEnd),
            VMTError::Expected(c) => f(VMTError::Expected(c)),
            VMTError::UnexpectedEof => f(VMTError::UnexpectedEof),
            VMTError::InvalidBlendMode(u) => f(VMTError::InvalidBlendMode(u)),
            VMTError::Utf8Parse(e) => f(VMTError::Utf8Parse(e)),
            VMTError::FloatParse(e) => f(VMTError::FloatParse(e)),
            VMTError::IntParse(e) => f(VMTError::IntParse(e)),
            VMTError::BoolParse(e) => f(VMTError::BoolParse(e)),
            VMTError::Other(e) => e,
        }
    }
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
impl std::error::Error for VMTError {}
impl std::fmt::Display for VMTError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VMTError::MissingShaderName => write!(f, "Missing shader name"),
            VMTError::NoStringStart => write!(f, "No string start"),
            VMTError::NoStringEnd => write!(f, "No string end"),
            VMTError::Expected(c) => write!(f, "Expected '{}'", c),
            VMTError::UnexpectedEof => write!(f, "Unexpected EOF"),
            VMTError::InvalidBlendMode(u) => write!(f, "Invalid blend mode: {}", u),
            VMTError::Utf8Parse(e) => write!(f, "Utf8 parse error: {}", e),
            VMTError::FloatParse(e) => write!(f, "Float parse error: {}", e),
            VMTError::IntParse(e) => write!(f, "Int parse error: {}", e),
            VMTError::BoolParse(e) => write!(f, "Bool parse error: {}", e),
            VMTError::Other(_e) => write!(f, "Other error"),
        }
    }
}

#[derive(Clone)]
pub enum ShaderName<'a> {
    String(Cow<'a, [u8]>),
    LightmappedGeneric,
    UnlitGeneric,
    VertexLitGeneric,
    // ?
    Water,
    Patch,
}
impl<'a> ShaderName<'a> {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ShaderName::String(s) => s,
            ShaderName::LightmappedGeneric => b"LightmappedGeneric",
            ShaderName::UnlitGeneric => b"UnlitGeneric",
            ShaderName::VertexLitGeneric => b"VertexLitGeneric",
            ShaderName::Water => b"Water",
            ShaderName::Patch => b"Patch",
        }
    }
}
impl<'a> From<&'a [u8]> for ShaderName<'a> {
    fn from(s: &[u8]) -> ShaderName {
        if s.eq_ignore_ascii_case(b"LightmappedGeneric") {
            ShaderName::LightmappedGeneric
        } else if s.eq_ignore_ascii_case(b"UnlitGeneric") {
            ShaderName::UnlitGeneric
        } else if s.eq_ignore_ascii_case(b"VertexLitGeneric") {
            ShaderName::VertexLitGeneric
        } else if s.eq_ignore_ascii_case(b"Water") {
            ShaderName::Water
        } else if s.eq_ignore_ascii_case(b"Patch") {
            ShaderName::Patch
        } else {
            // TODO: remove this
            panic!("Unknown shader name: {:?}", s);
            ShaderName::String(Cow::Borrowed(s))
        }
    }
}
impl<'a> PartialEq for ShaderName<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ShaderName::String(a), ShaderName::String(b)) => a.eq_ignore_ascii_case(b),
            (ShaderName::LightmappedGeneric, ShaderName::LightmappedGeneric) => true,
            (ShaderName::UnlitGeneric, ShaderName::UnlitGeneric) => true,
            (ShaderName::VertexLitGeneric, ShaderName::VertexLitGeneric) => true,
            (ShaderName::Water, ShaderName::Water) => true,
            (ShaderName::Patch, ShaderName::Patch) => true,
            (ShaderName::String(a), b) => a.eq_ignore_ascii_case(b.as_bytes()),
            (a, ShaderName::String(b)) => a.as_bytes().eq_ignore_ascii_case(b),
            _ => false,
        }
    }
}
impl<'a> Eq for ShaderName<'a> {}
impl std::fmt::Debug for ShaderName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShaderName::String(s) => write!(
                f,
                "String({:?})",
                std::str::from_utf8(s).unwrap_or("<invalid utf8>")
            ),
            ShaderName::LightmappedGeneric => write!(f, "LightmappedGeneric"),
            ShaderName::UnlitGeneric => write!(f, "UnlitGeneric"),
            ShaderName::VertexLitGeneric => write!(f, "VertexLitGeneric"),
            ShaderName::Water => write!(f, "Water"),
            ShaderName::Patch => write!(f, "Patch"),
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
    pub base_texture_transform: Option<Cow<'a, str>>,
    pub color: Option<RGB>,

    // TODO: detail texture transform
    pub phong: Option<f32>,
    pub phong_boost: Option<f32>,
    pub phong_exponent: Option<f32>,
    pub phong_fresnel_ranges: Option<[f32; 3]>,

    pub lightwarp_texture: Option<TextureStr<'a>>,

    pub keywords: Option<Cow<'a, str>>,

    pub include: Option<Cow<'a, str>>,

    // TODO: is this some sort of enum?
    pub other: VMTOther<'a>,
    pub sub: VMTSubs<'a>,
}
impl<'a> VMT<'a> {
    /// Apply another VMT ontop of this, overwriting any fields the other sets.  
    /// Currently skips shader name, unsure if that should change.
    pub fn apply<'b>(self, o: &VMT<'b>) -> VMT<'b>
    where
        'a: 'b,
    {
        VMT {
            shader_name: self.shader_name,
            base_texture: apply(self.base_texture, &o.base_texture),
            decal: o.decal.or(self.decal),
            surface_prop: apply(self.surface_prop, &o.surface_prop),
            detail: self.detail.apply(&o.detail),
            detail2: self.detail2.apply(&o.detail2),
            base_texture_transform: apply(self.base_texture_transform, &o.base_texture_transform),
            color: apply(self.color, &o.color),
            phong: o.phong.or(self.phong),
            phong_boost: o.phong_boost.or(self.phong_boost),
            phong_exponent: o.phong_exponent.or(self.phong_exponent),
            phong_fresnel_ranges: apply(self.phong_fresnel_ranges, &o.phong_fresnel_ranges),
            lightwarp_texture: apply(self.lightwarp_texture, &o.lightwarp_texture),
            keywords: apply(self.keywords, &o.keywords),
            include: apply(self.include, &o.include),
            other: {
                let mut other = self.other;
                other
                    .0
                    .extend(o.other.0.iter().map(|(k, v)| (k.clone(), v.clone())));
                other
            },
            sub: self.sub.apply(&o.sub),
        }
    }

    /// Resolve any include statements.  
    /// Must be given a function to load another vmt, it is then merged with this VMT.
    pub fn resolve<'b, E>(
        self,
        load: impl FnOnce(&str) -> Result<VMT<'b>, E>,
    ) -> Result<VMT<'b>, VMTError<E>>
    where
        'a: 'b,
    {
        let Some(include) = &self.include else {
            return Ok(self);
        };

        let vmt = load(include).map_err(VMTError::Other)?;

        let has_include = vmt.include.is_some();

        let mut vmt = vmt.apply(&self);

        if !has_include {
            // if it doesn't have an include beforehand, we just remove it from the resolved/merged
            // vmt.
            vmt.include = None;
        }

        Ok(vmt)
    }

    pub fn resolve_recurse<'b, E>(
        self,
        mut load: impl FnMut(&str) -> Result<VMT<'b>, E>,
    ) -> Result<VMT<'b>, VMTError<E>>
    where
        'a: 'b,
    {
        let mut vmt = self;
        loop {
            vmt = vmt.resolve(&mut load)?;
            if vmt.include.is_none() {
                break;
            }
        }

        Ok(vmt)
    }

    pub fn from_bytes(b: &'a [u8]) -> Result<VMT<'a>, VMTError> {
        let mut iter = vmt_from_bytes(b);
        let shader_name = iter.next().ok_or(VMTError::MissingShaderName)??;
        let VMTItem::ShaderName(shader_name) = shader_name else {
            return Err(VMTError::MissingShaderName);
        };

        let mut vmt = VMT::default();
        vmt.shader_name = shader_name;

        let mut sub_depth = 0;
        // we can't use the [T; 16] because it isn't Copy
        let mut sub_path: [Cow<'_, [u8]>; 16] =
            std::array::from_fn(|_| Cow::Borrowed(b"" as &[u8]));
        for v in iter {
            let v = v?;
            match v {
                VMTItem::ShaderName(_) => unreachable!(),
                VMTItem::KeyValue(k, val) => {
                    let val = std::str::from_utf8(val)?;

                    if sub_depth != 0 {
                        // We're in a sub
                        let mut sub = &mut vmt.sub;
                        // TODO(minor): this does more string allocs than it really needs to
                        for i in 0..sub_depth {
                            let sub_name = sub_path[i].clone();
                            let tmp = sub
                                .0
                                .entry(sub_name)
                                .or_insert_with(|| VMTSub::Sub(VMTSubs::default()));
                            match tmp {
                                VMTSub::Sub(s) => sub = s,
                                VMTSub::Val(_) => unreachable!(),
                            }
                        }

                        let key_name = to_lowercase_cow(k);
                        sub.0.insert(key_name, VMTSub::Val(Cow::Borrowed(val)));
                    }

                    // Root shader names that we recognize
                    if k.eq_ignore_ascii_case(b"$basetexture") {
                        vmt.base_texture = Some(Cow::Borrowed(val));
                    } else if k.eq_ignore_ascii_case(b"%keywords") {
                        vmt.keywords = Some(Cow::Borrowed(val));
                    } else if k.eq_ignore_ascii_case(b"$detail") {
                        vmt.detail.texture = Some(Cow::Borrowed(val));
                    } else if k.eq_ignore_ascii_case(b"$detailscale") {
                        vmt.detail.scale = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$detailblendmode") {
                        let val: u8 = val.parse()?;
                        let val = DetailBlendMode::try_from(val)
                            .map_err(|_| VMTError::InvalidBlendMode(val))?;
                        vmt.detail.blend_mode = Some(val);
                    } else if k.eq_ignore_ascii_case(b"$detailblendfactor") {
                        vmt.detail.blend_factor = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$surfaceprop") {
                        vmt.surface_prop = Some(Cow::Borrowed(val));
                    } else if k.eq_ignore_ascii_case(b"$decal") {
                        vmt.decal = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$basetexturetransform") {
                        vmt.base_texture_transform = Some(Cow::Borrowed(val));
                    } else if k.eq_ignore_ascii_case(b"$color") {
                        let (_, val) = take_vec3(val.as_bytes())?;
                        vmt.color = Some(val);
                    } else if k.eq_ignore_ascii_case(b"$detailtint") {
                        let (_, val) = take_vec3(val.as_bytes())?;
                        vmt.detail.tint = Some(val);
                    } else if k.eq_ignore_ascii_case(b"$detailframe") {
                        vmt.detail.frame = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$detailalphamaskbasetexture") {
                        vmt.detail.alpha_mask_base_texture = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$detail2") {
                        vmt.detail2.texture = Some(Cow::Borrowed(val));
                    } else if k.eq_ignore_ascii_case(b"$detailscale2") {
                        vmt.detail2.scale = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$detailblendfactor2") {
                        vmt.detail2.blend_factor = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$detailframe2") {
                        vmt.detail2.frame = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$detailtint2") {
                        let (_, val) = take_vec3(val.as_bytes())?;
                        vmt.detail2.tint = Some(val);
                    } else if k.eq_ignore_ascii_case(b"$phong") {
                        vmt.phong = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$phongboost") {
                        vmt.phong_boost = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$phongexponent") {
                        vmt.phong_exponent = Some(val.parse()?);
                    } else if k.eq_ignore_ascii_case(b"$phongfresnelranges") {
                        let (_, val) = take_vec3(val.as_bytes())?;
                        vmt.phong_fresnel_ranges = Some(val);
                    } else if k.eq_ignore_ascii_case(b"$lightwarptexture") {
                        vmt.lightwarp_texture = Some(Cow::Borrowed(val));
                    } else if k.eq_ignore_ascii_case(b"include") {
                        vmt.include = Some(Cow::Borrowed(val));
                    } else {
                        // Convert key name to lowercase, but only allocate a string if we *have* to
                        let key_name = to_lowercase_cow(k);

                        vmt.other.0.insert(key_name, Cow::Borrowed(val));
                    }
                }
                VMTItem::KeySub(sub_name) => {
                    let sub_name = to_lowercase_cow(sub_name);
                    sub_path[sub_depth] = sub_name;
                    sub_depth += 1;

                    // This is just to insert the empty sub
                    let mut sub = &mut vmt.sub;
                    // TODO(minor): this does more string allocs than it really needs to
                    for i in 0..sub_depth {
                        let sub_name = sub_path[i].clone();
                        let tmp = sub
                            .0
                            .entry(sub_name)
                            .or_insert_with(|| VMTSub::Sub(VMTSubs::default()));
                        match tmp {
                            VMTSub::Sub(s) => sub = s,
                            VMTSub::Val(_) => unreachable!(),
                        }
                    }
                }
                VMTItem::EndSub => {
                    sub_depth -= 1;
                    sub_path[sub_depth] = Cow::Borrowed(b"" as &[u8]);
                }
                VMTItem::Comment(_) => {}
            }
        }

        Ok(vmt)
    }
}
impl<'a> Default for VMT<'a> {
    fn default() -> VMT<'a> {
        VMT {
            shader_name: ShaderName::LightmappedGeneric,
            base_texture: None,
            keywords: None,
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
            include: None,
            other: VMTOther::default(),
            sub: VMTSubs::default(),
        }
    }
}

#[derive(Default, Clone, PartialEq)]
pub struct VMTSubs<'a>(pub HashMap<Cow<'a, [u8]>, VMTSub<'a>>);
impl<'a> VMTSubs<'a> {
    pub fn apply<'b>(self, _o: &VMTSubs<'b>) -> VMTSubs<'b>
    where
        'a: 'b,
    {
        // TODO: actually apply subs
        self
    }

    pub fn get(&self, key: impl AsRef<[u8]>) -> Option<&VMTSub<'a>> {
        self.0.get(key.as_ref())
    }
}
impl<'a> std::fmt::Debug for VMTSubs<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut map = f.debug_map();

        for (k, v) in &self.0 {
            let k = std::str::from_utf8(k).unwrap_or("<invalid utf8>");
            map.entry(&k, v);
        }

        map.finish()
    }
}

#[derive(Clone, PartialEq)]
pub enum VMTSub<'a> {
    Val(Cow<'a, str>),
    Sub(VMTSubs<'a>),
}
impl<'a> VMTSub<'a> {
    pub fn as_val(&self) -> Option<&str> {
        match self {
            VMTSub::Val(v) => Some(v),
            VMTSub::Sub(_) => None,
        }
    }

    pub fn as_sub(&self) -> Option<&VMTSubs<'a>> {
        match self {
            VMTSub::Val(_) => None,
            VMTSub::Sub(v) => Some(v),
        }
    }
}
impl<'a> std::fmt::Debug for VMTSub<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VMTSub::Val(v) => write!(f, "{v:?}",),
            VMTSub::Sub(v) => write!(f, "{v:?}"),
        }
    }
}

#[derive(Default, Clone, PartialEq)]
pub struct VMTOther<'a>(pub HashMap<Cow<'a, [u8]>, Cow<'a, str>>);
impl<'a> VMTOther<'a> {
    pub fn get(&self, key: impl AsRef<[u8]>) -> Option<&str> {
        self.0.get(key.as_ref()).map(|v| v.as_ref())
    }
}
impl<'a> std::fmt::Debug for VMTOther<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // We have to make the keys readable as str

        let mut map = f.debug_map();

        for (k, v) in &self.0 {
            let k = std::str::from_utf8(k).unwrap_or("<invalid utf8>");
            map.entry(&k, v);
        }

        map.finish()
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
impl<'a> VMTDetail<'a> {
    pub fn apply<'b>(self, o: &VMTDetail<'b>) -> VMTDetail<'b>
    where
        'a: 'b,
    {
        VMTDetail {
            texture: apply(self.texture, &o.texture),
            tint: o.tint.or(self.tint),
            frame: o.frame.or(self.frame),
            scale: o.scale.or(self.scale),
            alpha_mask_base_texture: o.alpha_mask_base_texture.or(self.alpha_mask_base_texture),
            blend_mode: o.blend_mode.or(self.blend_mode),
            blend_factor: o.blend_factor.or(self.blend_factor),
        }
    }
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
impl<'a> VMTDetail2<'a> {
    pub fn apply<'b>(self, o: &VMTDetail2<'b>) -> VMTDetail2<'b>
    where
        'a: 'b,
    {
        VMTDetail2 {
            texture: apply(self.texture, &o.texture),
            scale: o.scale.or(self.scale),
            blend_factor: o.blend_factor.or(self.blend_factor),
            frame: o.frame.or(self.frame),
            tint: o.tint.or(self.tint),
        }
    }
}

#[derive(Clone)]
pub enum VMTItem<'a> {
    /// `"LightmappedGeneric"`
    /// Key values are inside of the braces
    ShaderName(ShaderName<'a>),
    /// `"blah" "42"`
    KeyValue(&'a [u8], &'a [u8]),
    /// The start of a sub entry, e.g. `"blah" {}`
    /// Key values are inside of the braces
    KeySub(&'a [u8]),
    /// The end of a sub entry, e.g. `"blah" {}`
    EndSub,
    Comment(&'a [u8]),
}
impl<'a> VMTItem<'a> {
    pub fn as_shader_name(&self) -> Option<&ShaderName<'a>> {
        match self {
            VMTItem::ShaderName(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_key_value(&self) -> Option<(&[u8], &[u8])> {
        match self {
            VMTItem::KeyValue(k, v) => Some((k, v)),
            _ => None,
        }
    }

    pub fn as_key_sub(&self) -> Option<&[u8]> {
        match self {
            VMTItem::KeySub(k) => Some(k),
            _ => None,
        }
    }

    pub fn as_comment(&self) -> Option<&[u8]> {
        match self {
            VMTItem::Comment(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_end_sub(&self) -> Option<()> {
        match self {
            VMTItem::EndSub => Some(()),
            _ => None,
        }
    }

    pub fn into_shader_name(self) -> Option<ShaderName<'a>> {
        match self {
            VMTItem::ShaderName(s) => Some(s),
            _ => None,
        }
    }

    pub fn into_key_value(self) -> Option<(&'a [u8], &'a [u8])> {
        match self {
            VMTItem::KeyValue(k, v) => Some((k, v)),
            _ => None,
        }
    }

    pub fn into_key_sub(self) -> Option<&'a [u8]> {
        match self {
            VMTItem::KeySub(k) => Some(k),
            _ => None,
        }
    }

    pub fn into_comment(self) -> Option<&'a [u8]> {
        match self {
            VMTItem::Comment(c) => Some(c),
            _ => None,
        }
    }

    pub fn into_end_sub(self) -> Option<()> {
        match self {
            VMTItem::EndSub => Some(()),
            _ => None,
        }
    }
}
impl<'a> std::fmt::Debug for VMTItem<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VMTItem::ShaderName(s) => write!(f, "ShaderName({:?})", s),
            VMTItem::KeyValue(k, v) => write!(
                f,
                "KeyValue({:?}, {:?})",
                std::str::from_utf8(k).unwrap_or("<invalid utf8>"),
                std::str::from_utf8(v).unwrap_or("<invalid utf8>")
            ),
            VMTItem::KeySub(k) => write!(
                f,
                "KeySub({:?})",
                std::str::from_utf8(k).unwrap_or("<invalid utf8>")
            ),
            VMTItem::EndSub => write!(f, "EndSub"),
            VMTItem::Comment(c) => write!(
                f,
                "Comment({:?})",
                std::str::from_utf8(c).unwrap_or("<invalid utf8>")
            ),
        }
    }
}

/// Iterator over the items of the VMT, for if you only care about specific pieces and don't want
/// to do all of the parsing that [`VMT`] does.  
/// This does not allocate.
pub fn vmt_from_bytes<'a>(
    bytes: &'a [u8],
) -> impl Iterator<Item = Result<VMTItem<'a>, VMTError>> + '_ {
    let (mut b, shader_name) = match take_text(bytes) {
        Ok((b, shader_name)) => {
            let shader_name = ShaderName::from(shader_name);
            (b, Ok(VMTItem::ShaderName(shader_name)))
        }
        // Note: the unaltered `b` should never really be used because it would only have no value
        // if the shader name failed, which would never run main iter due to the StopOnErr adapter
        Err(err) => (bytes, Err(err)),
    };

    let shader_name = std::iter::once(shader_name);

    let mut is_first = true;
    let mut sub_depth = 0;

    let mut next = move || -> Result<Option<VMTItem<'a>>, VMTError> {
        if is_first {
            // If we just parsed the shader name, we have to grab the opening bracket
            b = take_whitespace(b)?;
            b = expect_char(b, b'{')?;

            is_first = false;
        }

        b = take_whitespace(b)?;

        if b.starts_with(b"}") {
            if sub_depth == 0 {
                // We're done with the top level
                // TODO: check whether there's actually nothing left?
                return Ok(None);
            } else {
                // We're done with a sub
                sub_depth -= 1;
                b = &b[1..];
                return Ok(Some(VMTItem::EndSub));
            }
        }

        if b.is_empty() {
            return Err(VMTError::UnexpectedEof);
        }

        // comment
        if b.starts_with(b"//") {
            let end = b
                .iter()
                .position(|&b| b == b'\n')
                .unwrap_or_else(|| b.len());
            let comment = &b[..end];
            b = &b[end..];
            return Ok(Some(VMTItem::Comment(comment)));
        }

        let (b2, key_name) = take_text(b)?;
        b = b2;

        b = take_whitespace(b)?;

        if b.starts_with(b"{") {
            // We're starting a sub
            sub_depth += 1;
            b = &b[1..];
            return Ok(Some(VMTItem::KeySub(key_name)));
        }

        // TODO: we could have a malformed value error which gives the name
        let (b2, val) = take_text(b)?;
        b = b2;

        return Ok(Some(VMTItem::KeyValue(key_name, val)));
    };

    let main_iter = std::iter::from_fn(move || next().transpose()).fuse();

    let iter = shader_name.chain(main_iter);

    StopOnErr::new(iter)
}

#[cfg(test)]
mod test {
    use std::borrow::Cow;

    use crate::{ShaderName, VMTSub, VMTSubs};

    use super::VMT;

    #[test]
    fn test_basic_vmt() {
        // Empty
        let text = r#""LightmappedGeneric" {}"#;
        let vmt = VMT::from_bytes(text.as_bytes()).unwrap();

        assert_eq!(vmt.shader_name, ShaderName::LightmappedGeneric);

        // Simple
        let text = r#""LightmappedGeneric"
        {
            "$basetexture" "Thing/thingy001"
            "$envmap" "env_cubemap"
            "$basealphaenvmapmask" 1
            "$surfaceprop" "metal"
            "%keywords" "test"
        }
        "#;

        let vmt = VMT::from_bytes(text.as_bytes()).unwrap();
        assert_eq!(vmt.shader_name, ShaderName::LightmappedGeneric);
        assert_eq!(vmt.base_texture, Some("Thing/thingy001".into()));
        assert_eq!(vmt.keywords, Some("test".into()));
        assert_eq!(vmt.other.get(b"$envmap" as &[u8]), Some("env_cubemap"));
        assert_eq!(vmt.other.get(b"$basealphaenvmapmask" as &[u8]), Some("1"));
        assert_eq!(vmt.surface_prop, Some("metal".into()));

        // Simple + Comments
        let text = r#""LightmappedGeneric"
        {
            "$basetexture" "Thing/thingy001"
            // "$envmap" "env_cubemap"
            "$basealphaenvmapmask" 1 // thingy
            "$surfaceprop" "metal"
            "%keywords" "test"
        }
        "#;

        let vmt = VMT::from_bytes(text.as_bytes()).unwrap();
        assert_eq!(vmt.shader_name, ShaderName::LightmappedGeneric);
        assert_eq!(vmt.base_texture, Some("Thing/thingy001".into()));
        assert_eq!(vmt.keywords, Some("test".into()));
        assert_eq!(vmt.other.get(b"$basealphaenvmapmask" as &[u8]), Some("1"));
        assert_eq!(vmt.surface_prop, Some("metal".into()));
    }

    #[test]
    fn test_sub_vmt() {
        let text = r#""Water"
        {
                "Water_DX60"
                {
                        "$fallbackmaterial" "nature/blah"
                }
        
                "Proxies"
                {
                        "AnimatedTexture"
                        {
                                "animatedtexturevar" "$normalmap"
                                "animatedtextureframenumvar" "$bumpframe"
                                "animatedtextureframerate" 24.00
                        }
        
                        "TextureScroll"
                        {
                                "texturescrollvar" "$bumptransform"
                                "texturescrollrate" .05
                                "texturescrollangle" 45.00
                        }
        
                        "WaterLOD"
                        {
                        }
                }
        }"#;

        let vmt = VMT::from_bytes(text.as_bytes()).unwrap();

        assert_eq!(vmt.shader_name, ShaderName::String(Cow::Borrowed(b"Water")));
        assert_eq!(vmt.sub.0.len(), 2);
        assert_eq!(
            vmt.sub.get(b"water_dx60"),
            Some(&VMTSub::Sub(VMTSubs {
                0: vec![(
                    Cow::Borrowed(b"$fallbackmaterial" as &[u8]),
                    VMTSub::Val("nature/blah".into())
                )]
                .into_iter()
                .collect()
            }))
        );

        let proxies = vmt.sub.get(b"proxies").unwrap().as_sub().unwrap();

        let animated_texture = proxies.get(b"animatedtexture").unwrap().as_sub().unwrap();

        assert_eq!(
            animated_texture.get(b"animatedtexturevar"),
            Some(&VMTSub::Val("$normalmap".into()))
        );

        assert_eq!(
            animated_texture.get(b"animatedtextureframenumvar"),
            Some(&VMTSub::Val("$bumpframe".into()))
        );

        assert_eq!(
            animated_texture.get(b"animatedtextureframerate"),
            Some(&VMTSub::Val("24.00".into()))
        );

        let texture_scroll = proxies.get(b"texturescroll").unwrap().as_sub().unwrap();

        assert_eq!(
            texture_scroll.get(b"texturescrollvar"),
            Some(&VMTSub::Val("$bumptransform".into()))
        );

        assert_eq!(
            texture_scroll.get(b"texturescrollrate"),
            Some(&VMTSub::Val(".05".into()))
        );

        assert_eq!(
            texture_scroll.get(b"texturescrollangle"),
            Some(&VMTSub::Val("45.00".into()))
        );

        assert_eq!(
            proxies.get(b"waterlod"),
            Some(&VMTSub::Sub(VMTSubs::default()))
        );
    }
}
