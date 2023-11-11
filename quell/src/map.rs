use std::{borrow::Cow, path::Path};

use bevy::{
    prelude::{Entity, Resource},
    utils::HashMap,
};
use vbsp::Bsp;

use crate::data::LSrc;

#[derive(Debug, Resource)]
pub struct GameMap {
    pub bsp: Bsp,
    /// Keeps track of the mapping between the face index in the current bsp map, and the face
    /// entities.
    pub faces: HashMap<usize, Entity>,
}
impl GameMap {
    pub fn from_path(path: impl AsRef<Path>) -> eyre::Result<GameMap> {
        let data = std::fs::read(path)?;
        let bsp = Bsp::read(&data)?;

        Ok(GameMap {
            bsp,
            faces: HashMap::new(),
        })
    }

    pub fn find_vmt(&self, name: &str) -> Option<(Vec<u8>, LSrc)> {
        // let zip = self.bsp.pack.zip.lock().unwrap();
        // for testing print the top level
        // for k in zip.file_names() {
        //     println!("- {k}");
        // }

        let name = if name.starts_with("materials/") && name.ends_with(".vmt") {
            Cow::Borrowed(name)
        } else if name.starts_with("materials/")
        /* && !name.ends_with(".vmt") */
        {
            Cow::Owned(format!("{}.vmt", name))
        } else {
            Cow::Owned(format!("materials/{}.vmt", name))
        };
        let res = self.bsp.pack.get(&name).unwrap()?;
        Some((res, LSrc::Map))
    }

    pub fn has_texture(&self, name: &str) -> bool {
        let name = if name.starts_with("materials/") && name.ends_with(".vtf") {
            Cow::Borrowed(name)
        } else if name.starts_with("materials/")
        /* && !name.ends_with(".vtf") */
        {
            Cow::Owned(format!("{}.vtf", name))
        } else {
            Cow::Owned(format!("materials/{}.vtf", name))
        };
        self.bsp.pack.contains(&name).unwrap_or(false)
    }

    // TODO: we could modify it to read texture data into a caller's buffer to more efficiently
    // reuse an allocation
    pub fn get_texture_data(&self, name: &str) -> Option<Vec<u8>> {
        let name = if name.starts_with("materials/") && name.ends_with(".vtf") {
            Cow::Borrowed(name)
        } else if name.starts_with("materials/")
        /* && !name.ends_with(".vtf") */
        {
            Cow::Owned(format!("{}.vtf", name))
        } else {
            Cow::Owned(format!("materials/{}.vtf", name))
        };
        let res = self.bsp.pack.get(&name).unwrap()?;
        Some(res)
    }
}
