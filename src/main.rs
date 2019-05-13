#[macro_use]
extern crate log;
extern crate env_logger;

use std::env;
use std::{fs,io};
use std::path::Path;
use std::path::PathBuf;
use std::ffi::OsStr;
use std::time::SystemTime;
use time::Timespec;
use libc::ENOENT;
use std::os::linux::fs::MetadataExt;

use fuse::{FileType, FileAttr, Filesystem, Request, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory};

const TTL: Timespec = Timespec { sec: 1, nsec: 0}; // 1 second

#[derive(Debug)]
struct PassthruFS {
    sourceroot: PathBuf
}

impl PassthruFS {
    fn new(sourceroot: &OsStr) -> PassthruFS {
      PassthruFS { sourceroot: PathBuf::from(sourceroot) }
    }

    fn stat(&self, path: &PathBuf) -> io::Result<FileAttr> {
      let attr = fs::metadata(path)?;
      Ok(FileAttr {
        ino: attr.st_ino(),
        size: attr.st_size(),
        blocks: attr.st_blocks(),
        atime: Timespec {sec: attr.st_atime(), nsec: attr.st_atime_nsec() as i32},
        mtime: Timespec {sec: attr.st_mtime(), nsec: attr.st_mtime_nsec() as i32},
        ctime: Timespec {sec: attr.st_ctime(), nsec: attr.st_ctime_nsec() as i32},
        crtime: Timespec {sec: 0, nsec: 0},
        kind: FileType::Directory,
        perm: attr.st_mode() as u16,
        nlink: attr.st_nlink() as u32,
        uid: attr.st_uid(),
        gid: attr.st_gid(),
        rdev: attr.st_rdev() as u32,
        flags: 0,
      })
    }
}

impl Filesystem for PassthruFS {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        info!("getattr {:?}", ino);
        match ino {
            1 => reply.attr(&TTL, &self.stat(&self.sourceroot).unwrap()),
            _ => reply.error(ENOENT)
        }
    }
}

fn main() {
  env_logger::init();

    let mountpoint = env::args_os().nth(1).unwrap();
    let sourceroot = env::args_os().nth(2).unwrap();

    let fs = PassthruFS::new(&sourceroot);
    let options = ["-o", "ro", "-o", "fsname=hello", "-o", "allow_other"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    fuse::mount(fs, &mountpoint, &options).unwrap();
}
