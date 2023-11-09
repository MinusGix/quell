use std::io::Cursor;

use memmap2::Mmap;
use vpk::entry::VpkReaderProvider;

pub struct MemmapVPKProv {
    files: Vec<Mmap>,
}
impl MemmapVPKProv {
    pub fn new<'s>(paths: impl Iterator<Item = &'s str>) -> std::io::Result<MemmapVPKProv> {
        let files = paths
            .into_iter()
            .map(|path| {
                let file = std::fs::File::open(path)?;
                // TODO: lock the file?
                // Safety: This isn't completely safe, because it could be modified from under us,
                // but that is very unlikely.
                let mmap = unsafe { memmap2::Mmap::map(&file)? };
                Ok(mmap)
            })
            .collect::<std::io::Result<Vec<_>>>()?;

        Ok(MemmapVPKProv { files })
    }
}
impl VpkReaderProvider for MemmapVPKProv {
    type Reader<'a> = Cursor<&'a [u8]>;

    fn vpk_reader<'a>(&'a self, archive_index: u16) -> std::io::Result<Option<Self::Reader<'a>>> {
        let file = &self.files[usize::from(archive_index)];
        Ok(Some(Cursor::new(&file[..])))
    }
}
