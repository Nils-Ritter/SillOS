use limine::file::File;

trait FileSystem{
    fn open(&self, path: &str) -> File;
    fn read(&self, file: &File, buf: &mut [u8]) -> usize;
}
