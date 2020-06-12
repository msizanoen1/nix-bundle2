use anyhow::Context;
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getegid, geteuid, pivot_root};
use std::convert::AsRef;
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

struct AutoUnmount {
    inner: Option<TempDir>,
    defused: bool,
}

impl AutoUnmount {
    fn new(inner: TempDir) -> Self {
        Self {
            inner: Some(inner),
            defused: false,
        }
    }

    #[allow(dead_code)]
    fn defuse(&mut self) {
        self.defused = true;
    }

    #[allow(dead_code)]
    fn take(mut self) -> TempDir {
        self.defuse();
        let dir = self.inner.take().unwrap();
        dir
    }
}

impl std::ops::Deref for AutoUnmount {
    type Target = TempDir;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for AutoUnmount {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap()
    }
}

impl Drop for AutoUnmount {
    fn drop(&mut self) {
        if !self.defused {
            safe_umount(self.inner.as_ref().unwrap().path()).unwrap();
        }
    }
}

fn safe_umount<F: AsRef<Path> + ?Sized>(path: &F) -> Result<(), anyhow::Error> {
    mount(
        None::<&str>,
        path.as_ref(),
        None::<&str>,
        MsFlags::MS_SLAVE | MsFlags::MS_REC,
        None::<&str>,
    )?;
    umount2(path.as_ref(), MntFlags::MNT_DETACH)?;
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let args = env::args().skip(1);
    let rest = args.collect::<Vec<_>>();
    let exedir = env::current_exe()?.parent().unwrap().to_owned();
    let dir = PathBuf::from("/.oldroot").join(exedir.join("usr").join("lib").strip_prefix("/")?);
    let cmd = fs::read_to_string(exedir.join("nixon_command.txt"))?;
    let uid = geteuid();
    let gid = getegid();
    let new_root = tempdir()?;
    if !uid.is_root() {
        unshare(CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWUSER)?;
        fs::File::create("/proc/self/setgroups")
            .context("Open setgroups")?
            .write_all(b"deny")
            .context("Deny setgroups")?;
        fs::File::create("/proc/self/uid_map")
            .context("Open uid_map")?
            .write_all(format!("{} {} 1", uid, uid).as_bytes())
            .context("Set uid map")?;
        fs::File::create("/proc/self/gid_map")
            .context("Open gid_map")?
            .write_all(format!("{} {} 1", gid, gid).as_bytes())
            .context("Set gid map")?;
    } else {
        unshare(CloneFlags::CLONE_NEWNS)?;
    }

    create_intermediate_mnt(new_root.path())?;
    mount(
        Some("nixuserchrootfs"),
        new_root.path(),
        Some("tmpfs"),
        MsFlags::empty(),
        Some("mode=0755"),
    )
    .context("Mounting tmpfs")?;
    let new_root = AutoUnmount::new(new_root);
    fs::create_dir(new_root.path().join(".oldroot")).context("Create .oldroot")?;
    let old_cwd = env::current_dir()?;
    pivot_root(new_root.path(), &new_root.path().join(".oldroot")).context("Set root directory")?;
    safe_umount(&PathBuf::from("/.oldroot").join(new_root.path().strip_prefix("/")?))?;
    fs::remove_dir(
        PathBuf::from("/.oldroot").join(new_root.take().into_path().strip_prefix("/")?),
    )?;
    setup_mounts(&dir).context("Setup mounts")?;
    env::set_current_dir(&old_cwd)?;
    safe_umount("/.oldroot")?;
    fs::remove_dir("/.oldroot")?;
    Err(Command::new(&cmd).args(&rest).exec().into())
}

fn create_intermediate_mnt<F: AsRef<Path> + ?Sized>(path: &F) -> Result<(), anyhow::Error> {
    mount(
        Some(path.as_ref()),
        path.as_ref(),
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )?;
    mount(
        None::<&str>,
        path.as_ref(),
        None::<&str>,
        MsFlags::MS_SLAVE | MsFlags::MS_REC,
        None::<&str>,
    )?;
    Ok(())
}

fn bind_mount<F: AsRef<Path> + ?Sized, T: AsRef<Path> + ?Sized>(
    from: &F,
    to: &T,
) -> Result<(), anyhow::Error> {
    let from = from.as_ref();
    let to = to.as_ref();
    let _ = umount2(to, MntFlags::MNT_DETACH);
    let _ = fs::remove_file(to);
    if let Ok(dest) = fs::read_link(&from) {
        unix_fs::symlink(&dest, &to).context("Replicate symlink")?;
        return Ok(());
    } else if from.is_dir() {
        fs::create_dir_all(to).context("Create directory for bind mount")?;
    } else {
        fs::File::create(to).context("Create file for bind mount")?;
    }
    mount(
        Some(from),
        to,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .context("Mount bind")?;
    Ok(())
}

fn merge_store<F: AsRef<Path> + ?Sized, T: AsRef<Path> + ?Sized>(
    from: &F,
    to: &T,
) -> Result<(), anyhow::Error> {
    let from = from.as_ref();
    let to = to.as_ref();
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let from = from.join(&name);
        let to = to.join(&name);
        bind_mount(&from, &to)?;
    }
    Ok(())
}

fn setup_mounts<T: AsRef<Path> + ?Sized>(store: &T) -> Result<(), anyhow::Error> {
    if fs::metadata("/.oldroot/nix")
        .map(|x| x.is_dir())
        .unwrap_or(false)
    {
        bind_mount("/.oldroot/nix", "/nix")?;
        mount(
            None::<&str>,
            "/nix",
            None::<&str>,
            MsFlags::MS_SLAVE | MsFlags::MS_REC,
            None::<&str>,
        )?;
        mount(
            Some("nixstorefs"),
            "/nix/store",
            Some("tmpfs"),
            MsFlags::empty(),
            Some("mode=0555"),
        )
        .context("nixstorefs")?;
        merge_store("/.oldroot/nix/store", "/nix/store")?;
    } else {
        fs::create_dir_all("/nix/store")?;
    }
    merge_store(&store, "/nix/store")?;
    let old_root = PathBuf::from("/.oldroot");
    let root = PathBuf::from("/");
    for entry in fs::read_dir(&old_root)? {
        let entry = entry?;
        let name = entry.file_name();
        match name.to_str() {
            Some("nix") | Some(".oldroot") => continue,
            _ => (),
        }
        let from = old_root.join(&name);
        let to = root.join(&name);
        bind_mount(&from, &to).context("Create bind mount")?;
    }
    Ok(())
}
