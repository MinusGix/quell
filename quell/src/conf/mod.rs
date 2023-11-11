use bevy::prelude::Resource;

use crate::cheats_all;

pub mod cheat;

// TODO: derive macro which generates:
// - 'true name', like `MatRenderConfig::leafvis`'s name is `mat_leafvis`
// - whether the field requires cheats (which is decided by the type)
// - allows specifying the default value (default impl will be `Default::default()`)

// TODO: general console variable specification that can be read from file?
// because we might not want all of them in this for every game?

#[derive(Debug, Default, Clone, Resource)]
pub struct Config {
    pub render: RenderConfig,
}

#[derive(Debug, Default, Clone)]
pub struct RenderConfig {
    // TODO: does no_vis or lock_pvs need sv_cheats?
    /// `r_novis`
    /// Disables using PVS to cull objects.
    pub no_vis: bool,
    /// `r_lockpvs`
    /// Prevents PVS from being recalculated.
    pub lock_pvs: bool,
    pub mat: MatRenderConfig,
}

#[derive(Debug, Default, Clone)]
pub struct MatRenderConfig {
    /// `mat_leafvis`
    pub leafvis: MatLeafvis,
}

/// The level of visleaf visualization to use.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum MatLeafvis {
    #[default]
    Off = 0,
    /// Draw the visleaf that the camera is in as a wireframe.
    CurrentVisleaf = 1,
    /// Draw the viscluster (often just equivalent to the visleaf) as a wireframe.
    CurrentViscluster = 2,
    /// Draw all visleaves as wireframes.
    /// Unaffected by `r_lockpvs`.
    AllVisleaves = 3,
    // TODO: Draw every single visleaf, even the ones that aren't in the current pvs?
}
cheats_all!(MatLeafvis);
