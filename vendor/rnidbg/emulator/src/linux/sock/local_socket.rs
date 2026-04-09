use crate::emulator::{AndroidEmulator, VMPointer};
use crate::linux::errno::Errno;
use crate::linux::file_system::{FileIO, FileIOTrait, SeekResult, StMode};
use crate::linux::structs::socket::Pf;
use crate::linux::structs::OFlag;

pub struct LocalSocket {
    path: Option<String>,
}

impl LocalSocket {
    pub fn new() -> Self {
        LocalSocket { path: None }
    }
}

impl<T: Clone> FileIOTrait<T> for LocalSocket {
    fn connect(
        &mut self,
        addr: VMPointer<T>,
        addr_len: usize,
        emulator: &AndroidEmulator<T>,
    ) -> i32 {
        let sa_family = emulator.backend.mem_read_v2::<u16>(addr.addr).unwrap();
        if sa_family != (Pf::LOCAL as u32) as u16 {
            emulator.set_errno(Errno::EINVAL.as_i32()).unwrap();
            return Errno::EINVAL.as_i32();
        }

        let path = emulator.backend.mem_read_c_string(addr.addr + 2).unwrap();

        if path.starts_with("/dev/socket/logd") {
            self.path = Some(path);
            return 0;
        }

        emulator.set_errno(Errno::EACCES.as_i32()).unwrap();
        return Errno::EACCES.as_i32();

    }

    fn close(&mut self) {}

    fn read(&mut self, buf: VMPointer<T>, count: usize) -> usize {
        0
    }

    fn pread(&mut self, buf: VMPointer<T>, count: usize, offset: usize) -> usize {
        0
    }

    fn write(&mut self, buf: &[u8]) -> i32 {
        if self.path.is_some() {
            buf.len() as i32
        } else {
            -1
        }
    }

    fn lseek(&mut self, offset: i64, whence: i32) -> SeekResult {
        SeekResult::UnknownError
    }

    fn path(&self) -> &str {
        self.path.as_deref().unwrap_or("/dev/socket/logdw")
    }

    fn oflags(&self) -> OFlag {
        OFlag::O_RDWR
    }

    fn st_mode(&self) -> StMode {
        StMode::S_IRUSR | StMode::S_IWUSR
    }

    fn uid(&self) -> i32 {
        0
    }

    fn len(&self) -> usize {
        0
    }

    fn to_vec(&mut self) -> Vec<u8> {
        Vec::new()
    }
}
