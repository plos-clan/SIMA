use log::{error, info};
use nix::NixPath;
use nix::mount::{MsFlags, mount};
use nix::sys::statfs::{FsType, PROC_SUPER_MAGIC, statfs};
use std::path::{Path, PathBuf};
use std::{fs, io};

pub struct MountService;

impl MountService {
    fn check_create(path: impl AsRef<Path>) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn is_mount(path: impl AsRef<Path>, fs_type: FsType) -> nix::Result<bool> {
        let info = statfs(path.as_ref())?;
        Ok(info.filesystem_type() == fs_type)
    }

    fn try_mount_fs(path: impl AsRef<Path>, fs: impl NixPath) -> bool {
        if let Err(err) = Self::check_create(path.as_ref()) {
            error!("error: failed to create proc {err}");
            return false;
        }
        match Self::is_mount(path.as_ref(), PROC_SUPER_MAGIC) {
            Ok(false) => {}
            Err(err) => {
                eprintln!("error: failed check proc mount: {err}");
                return false;
            }
            _ => return true,
        }
        if let Err(err) = mount(
            Some(&fs),
            path.as_ref(),
            Some(&fs),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None::<&str>,
        ) {
            eprintln!("error: failed mount {err}");
            return false;
        }
        true
    }

    pub fn mount() -> Self {
        if !Self::try_mount_fs("/proc", PathBuf::from("proc")) {
            std::process::exit(1);
        }
        info!("mounted /proc to proc");

        if !Self::try_mount_fs("/sys", PathBuf::from("sysfs")) {
            std::process::exit(1);
        }
        info!("mounted /sys to sysfs");

        MountService {}
    }
}
