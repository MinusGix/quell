use std::path::Path;

pub struct VpkData {
    pub data: vpk::VPK,
}
impl VpkData {
    // TODO: use paths
    pub fn load(path: &str) -> Result<VpkData, vpk::Error> {
        let data = vpk::from_path(path)?;
        Ok(VpkData { data })
    }

    pub fn find_ignore_case(&self, name: &str) -> Option<&vpk::entry::VPKEntry> {
        for (file, entry) in self.data.tree.iter() {
            if file.eq_ignore_ascii_case(name) {
                return Some(entry);
            }
        }

        None
    }
}
