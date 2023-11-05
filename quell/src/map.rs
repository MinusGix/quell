use std::{borrow::Cow, path::Path};

use bevy::prelude::Resource;
use vbsp::Bsp;

use crate::data::LSrc;

#[derive(Debug, Resource)]
pub struct GameMap {
    pub bsp: Bsp,
}
impl GameMap {
    pub fn from_path(path: impl AsRef<Path>) -> eyre::Result<GameMap> {
        let data = std::fs::read(path)?;
        let bsp = Bsp::read(&data)?;

        Ok(GameMap { bsp })
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
        println!("Full name: {name:?}");
        let res = self.bsp.pack.get(&name).unwrap()?;
        Some((res, LSrc::Map))
    }

    pub fn find_texture(&self, name: &str) -> Option<(Vec<u8>, LSrc)> {
        // let zip = self.bsp.pack.zip.lock().unwrap();
        // for testing print the top level
        // for k in zip.file_names() {
        //     println!("- {k}");
        // }

        // let name = format!("materials/{}.vtf", name);
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
        Some((res, LSrc::Map))
    }
}
