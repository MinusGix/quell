use std::{borrow::Cow, collections::HashMap};

#[derive(Debug, Clone)]
pub enum VMTError<E = ()> {
    NoStringStart,
    NoStringEnd,

    Expected(char),

    InvalidBlendMode(u8),

    Utf8Parse(std::str::Utf8Error),
    FloatParse(std::num::ParseFloatError),
    IntParse(std::num::ParseIntError),
    BoolParse(std::str::ParseBoolError),

    Other(E),
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
    // Patch?
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

    pub keywords: Option<Cow<'a, str>>,

    pub include: Option<Cow<'a, str>>,

    // TODO: is this some sort of enum?
    pub other: HashMap<Cow<'a, str>, &'a str>,
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
                other.extend(o.other.iter().map(|(k, v)| (k.clone(), *v)));
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
        let (b, shader_name) = take_text(b)?;
        let shader_name = ShaderName::from(shader_name);

        let b = take_whitespace(b)?;

        let b = expect_char(b, b'{')?;

        let b = take_whitespace(b)?;

        let mut vmt = VMT::default();
        vmt.shader_name = shader_name;

        let mut b_out = b;
        loop {
            let b = b_out;
            if b.starts_with(b"}") {
                break;
            }

            if b.is_empty() {
                return Err(VMTError::Expected('}'));
            }

            if b.starts_with(b"//") {
                // comment
                let end = b
                    .iter()
                    .position(|&b| b == b'\n')
                    .unwrap_or_else(|| b.len());
                let b = &b[end..];
                let b = take_whitespace(b)?;
                b_out = b;
                continue;
            }

            let (b, key_name) = take_text(b)?;

            let b = take_whitespace(b)?;

            if b.starts_with(b"{") {
                let b = parse_sub(b, key_name, &mut vmt.sub)?;

                b_out = b;

                continue;
            }

            let (b, val) = take_text(b)?;

            let b = take_whitespace(b)?;

            // TODO: add checks for duplicate key values?
            let k = key_name.as_bytes();
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
                let val =
                    DetailBlendMode::try_from(val).map_err(|_| VMTError::InvalidBlendMode(val))?;
                vmt.detail.blend_mode = Some(val);
            } else if k.eq_ignore_ascii_case(b"$detailblendfactor") {
                vmt.detail.blend_factor = Some(val.parse()?);
            } else if k.eq_ignore_ascii_case(b"$surfaceprop") {
                vmt.surface_prop = Some(Cow::Borrowed(val));
            } else if k.eq_ignore_ascii_case(b"$decal") {
                vmt.decal = Some(val.parse()?);
            } else if k.eq_ignore_ascii_case(b"$basetexturetransform") {
                let (_, val) = take_vec2(val.as_bytes())?;
                vmt.base_texture_transform = Some(val);
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
                let key_name = to_lowercase_cow(key_name);

                vmt.other.insert(key_name, val);
            }

            b_out = b;
        }

        let b = b_out;

        let b = expect_char(b, b'}')?;

        // typically should be empty by now.
        let b = take_whitespace(b)?;
        assert!(b.is_empty(), "b: {:?}", std::str::from_utf8(b));

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
            other: HashMap::new(),
            sub: VMTSubs::default(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct VMTSubs<'a>(pub HashMap<Cow<'a, str>, VMTSub<'a>>);
impl<'a> VMTSubs<'a> {
    pub fn apply<'b>(self, _o: &VMTSubs<'b>) -> VMTSubs<'b>
    where
        'a: 'b,
    {
        // TODO: actually apply subs
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VMTSub<'a> {
    Val(Cow<'a, str>),
    Sub(VMTSubs<'a>),
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

fn apply<T: Clone>(a: Option<T>, b: &Option<T>) -> Option<T> {
    if let Some(b) = b {
        Some(b.clone())
    } else {
        a
    }
}

fn parse_sub<'a>(b: &'a [u8], key: &'a str, root: &mut VMTSubs<'a>) -> Result<&'a [u8], VMTError> {
    let b = expect_char(b, b'{')?;
    let b = take_whitespace(b)?;

    let mut sub = VMTSubs::default();

    let mut b_cur = b;
    loop {
        let b = b_cur;

        if b.starts_with(b"}") {
            break;
        }

        if b.is_empty() {
            return Err(VMTError::Expected('}'));
        }

        let (b, key_name) = take_text(b)?;

        let b = take_whitespace(b)?;

        if b.starts_with(b"{") {
            let b = parse_sub(b, key_name, &mut sub)?;

            b_cur = b;

            continue;
        }

        let (b, val) = take_text(b)?;

        let b = take_whitespace(b)?;

        let key_name = to_lowercase_cow(key_name);

        sub.0.insert(key_name, VMTSub::Val(Cow::Borrowed(val)));

        let b = take_whitespace(b)?;

        b_cur = b;
    }

    let b = b_cur;
    let b = expect_char(b, b'}')?;
    let b = take_whitespace(b)?;

    let key = to_lowercase_cow(key);
    root.0.insert(key, VMTSub::Sub(sub));

    Ok(b)
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

fn to_lowercase_cow(text: &str) -> Cow<'_, str> {
    if text.chars().any(|c| c.is_ascii_uppercase()) {
        Cow::Owned(text.to_ascii_lowercase())
    } else {
        Cow::Borrowed(text)
    }
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
