#[macro_use]
extern crate log;
extern crate env_logger;

use std::env;
use std::{fs,io};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::ffi::{OsStr,OsString};
use std::time::SystemTime;
use time::Timespec;
use libc::ENOENT;
use std::os::linux::fs::MetadataExt;
use std::collections::HashMap;
use fuse::{FileType, FileAttr, Filesystem, Request, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory};

const TTL: Timespec = Timespec { sec: 1, nsec: 0}; // 1 second

#[derive(Debug)]
struct PassthruFS {
    sourceroot: PathBuf,
    inodes: HashMap<u64, OsString>
}

impl PassthruFS {
    fn new(sourceroot: &OsStr) -> PassthruFS {
      PassthruFS { sourceroot: PathBuf::from(sourceroot), inodes: HashMap::new() }
    }

    fn stat(&self, path: &PathBuf) -> io::Result<FileAttr> {
      info!("stat {:?}", path);
      let attr = fs::metadata(path)?;
      
      let file_type = match attr.is_dir() {
        true => FileType::Directory,
        false => FileType::RegularFile
      };
      let file_attr = FileAttr {
        ino: attr.st_ino(),
        size: attr.st_size(),
        blocks: attr.st_blocks(),
        atime: Timespec {sec: attr.st_atime(), nsec: attr.st_atime_nsec() as i32},
        mtime: Timespec {sec: attr.st_mtime(), nsec: attr.st_mtime_nsec() as i32},
        ctime: Timespec {sec: attr.st_ctime(), nsec: attr.st_ctime_nsec() as i32},
        crtime: Timespec {sec: 0, nsec: 0},
        kind: file_type,
        perm: attr.st_mode() as u16,
        nlink: attr.st_nlink() as u32,
        uid: attr.st_uid(),
        gid: attr.st_gid(),
        rdev: attr.st_rdev() as u32,
        flags: 0,
      };
      info!("file_attr {:?}", file_attr);
      Ok(file_attr)
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

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        info!("lookup {} {:?}", parent, name);
        match &self.stat(&self.sourceroot.join(name)) {
            Ok(stat) => {self.inodes.insert(stat.ino, name.to_os_string());reply.entry(&TTL, stat, 0)},
            _ => reply.error(ENOENT)
        };
        //reply.entry(&TTL, &self.stat(&self.sourceroot.join(name)).unwrap(), 0);
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        info!("readdir {} {}", ino, offset);
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }
        let mut entries = vec![ (1, FileType::Directory, String::from(".")), (1, FileType::Directory, String::from("..")) ];
        for entry in fs::read_dir(&self.sourceroot).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let attr = fs::metadata(&path).unwrap();
            let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
            let file_type = match attr.is_dir() {
                true => FileType::Directory,
                false => FileType::RegularFile
            };
            
            entries.push((attr.st_ino(), file_type, file_name));
        }
        info!("entries: {:?}", entries);
        
        // Offset of 0 means no offset.
        // Non-zero offset means the passed offset has already been seen, and we should start after
        // it.
        let to_skip = if offset == 0 { offset } else { offset + 1 } as usize;
        for (i, entry) in entries.into_iter().enumerate().skip(to_skip) {
            info!("reply {}, {}, {:?}, {}", entry.0, i as i64, entry.1, entry.2);
            let r = reply.add(entry.0, i as i64, entry.1, entry.2);
            info!("r {}", r);
        }
        reply.ok();
    }

    fn read(&mut self, _req: &Request, ino: u64, fh: u64, offset: i64, size: u32, reply: ReplyData) {
        info!("read(ino={}, fh={}, offset={}, size={})", ino, fh, offset, size);
        info!("    {:?}", self.inodes.get(&ino));
        match self.inodes.get(&ino) {
            Some(path) => {
                let mut f = File::open(&self.sourceroot.join(path)).unwrap();
                f.seek(SeekFrom::Start(offset as u64));
                let mut handle = f.take(size.into());
                let mut buffer = Vec::new();
                handle.read_to_end(&mut buffer);
                reply.data(&buffer);
                /*
                let bytes: &[u8] = &fs::read(&self.sourceroot.join(path)).unwrap();
                reply.data(&bytes[(offset as usize)..((offset+size as i64) as usize)]);
                */
            },
            _ => reply.error(ENOENT)
        };
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
