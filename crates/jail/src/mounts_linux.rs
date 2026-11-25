//! The program's view of the files, and its loopback.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::path::Path;

use rustix::fs::statfs;
use rustix::mount::{
    MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_bind, mount_bind_recursive, mount_change,
    mount_remount, unmount,
};
use rustix::process::{chdir, pivot_root};

use crate::plan_linux::Plan;

/// Makes the program's view of the files: a jail's whole root, or the host's with some paths emptied, then enters dir.
pub(crate) fn set_up_filesystem(p: &Plan, dir: &str) -> Result<(), String> {
    // Nothing mounted here may reach the host's mount namespace through shared propagation.
    mount_change("/", MountPropagationFlags::REC | MountPropagationFlags::PRIVATE)
        .map_err(|e| format!("make mounts private: {e}"))?;
    if p.jail.is_none() {
        for holder in holders(p.remount.iter().chain(&p.hide), "/") {
            pin(&holder)?;
        }
        // Read-only first: a path inside a hidden directory is then hidden with it.
        for path in &p.remount {
            read_only_in_place(path)?;
        }
        for path in &p.hide {
            hide("", path)?;
        }
    } else {
        build_root(p, dir)?;
    }
    chdir(if dir.is_empty() { "/" } else { dir }).map_err(|e| format!("enter {dir}: {e}"))
}

/// Assembles the jail's root on its mount point and makes it the root.
fn build_root(p: &Plan, dir: &str) -> Result<(), String> {
    let Some(j) = &p.jail else { return Ok(()) };
    mount(
        "tmpfs",
        j.root.as_str(),
        "tmpfs",
        MountFlags::NOSUID | MountFlags::NODEV,
        c"size=1m,mode=0755",
    )
    .map_err(|e| format!("mount the root: {e}"))?;
    // Parents sort before their children, so a path mounted under another lands on top of it: a read-only path inside
    // the work directory stays read-only. At the same path the read-only mount goes last, so it wins there too.
    // The directories in a writable path above a read-only or hidden path are bound too, writable, so that they are mount
    // points the program cannot rename.
    let tops: Vec<&str> = std::iter::once(dir)
        .chain(j.writable.iter().map(String::as_str))
        .collect();
    let pinned: BTreeSet<String> = tops
        .iter()
        .flat_map(|top| holders(j.read_only.iter().chain(&p.hide).filter(|p| within(p, top)), top))
        .collect();
    let mut mounts: Vec<(&str, bool)> = j.read_only.iter().map(|p| (p.as_str(), false)).collect();
    mounts.extend(tops.iter().map(|t| (*t, true)));
    mounts.extend(pinned.iter().map(|h| (h.as_str(), true)));
    mounts.sort_by_key(|&(path, writable)| (path, std::cmp::Reverse(writable)));
    for (path, writable) in mounts {
        bind_into(&j.root, path, writable)?;
    }
    for dev in ["/dev/null", "/dev/zero", "/dev/full", "/dev/random", "/dev/urandom"] {
        bind_device(&j.root, dev)?;
    }
    if let Some(socket) = &j.socket {
        bind_socket(&j.root, socket)?;
    }
    for path in &p.hide {
        hide(&j.root, path)?;
    }
    pivot(&j.root)
}

/// Shows the host's path at the same path under root, read-only unless writable. A path the host does not have is
/// skipped for a read-only mount, as a list of system paths holds some a host lacks, and is an error for the work dir.
fn bind_into(root: &str, path: &str, writable: bool) -> Result<(), String> {
    let info = match std::fs::metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound && !writable => return Ok(()),
        other => other.map_err(|e| format!("mount {path}: {e}"))?,
    };
    let target = format!("{root}{path}");
    mount_point(&target, info.is_dir()).map_err(|e| format!("mount {path}: {e}"))?;
    mount_bind(path, target.as_str()).map_err(|e| format!("mount {path}: {e}"))?;
    let mut flags = MountFlags::NOSUID | MountFlags::NODEV;
    if !writable {
        flags |= MountFlags::RDONLY;
    }
    remount(&target, flags).map_err(|e| format!("mount {path}: {e}"))
}

/// Sets flags on the bind mount at target, together with the flags it inherited, which the kernel does not let a remount
/// drop.
fn remount(target: &str, flags: MountFlags) -> Result<(), String> {
    let inherited = inherited_flags(target)?;
    mount_remount(target, MountFlags::BIND | inherited | flags, "").map_err(|e| e.to_string())
}

/// The directories above paths that lie strictly inside top and outside every one of paths, parents first, among those
/// the host has. Moving one would carry its path along and free the name for a copy the program made: the kernel refuses
/// to rename or remove a mount point, so each becomes one. A directory inside one of paths needs none: a read-only mount
/// refuses the rename, a hidden one shows nothing, and a writable mount there would undo the read-only one.
fn holders<'a>(paths: impl Iterator<Item = &'a String> + Clone, top: &str) -> BTreeSet<String> {
    paths
        .clone()
        .flat_map(|p| Path::new(p).ancestors().skip(1))
        .filter_map(Path::to_str)
        .filter(|h| *h != top && within(h, top) && !paths.clone().any(|p| within(h, p)))
        .filter(|h| Path::new(h).is_dir())
        .map(str::to_owned)
        .collect()
}

/// Binds the directory over itself, with what is mounted below it, so it is a mount point and reads as it did.
fn pin(dir: &str) -> Result<(), String> {
    mount_bind_recursive(dir, dir).map_err(|e| format!("pin {dir}: {e}"))
}

/// Says whether path is top or under it.
fn within(path: &str, top: &str) -> bool {
    path == top || top == "/" || path.strip_prefix(top).is_some_and(|rest| rest.starts_with('/'))
}

/// Makes the path read-only where it is: a bind mount of the path over itself, remounted read-only. A directory is
/// bound with what is mounted below it, so that its contents read as they did. A path that is not there is skipped.
fn read_only_in_place(path: &str) -> Result<(), String> {
    let info = match std::fs::metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        other => other.map_err(|e| format!("read-only {path}: {e}"))?,
    };
    let bound = if info.is_dir() {
        mount_bind_recursive(path, path)
    } else {
        mount_bind(path, path)
    };
    bound.map_err(|e| format!("read-only {path}: {e}"))?;
    remount(path, MountFlags::RDONLY).map_err(|e| format!("read-only {path}: {e}"))
}

/// Shows one of the host's device files at the same path under root; a device the host lacks is skipped. The mount keeps
/// the host's flags: a device file needs the nodev that bind_into would add to be absent.
fn bind_device(root: &str, path: &str) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Ok(());
    }
    let target = format!("{root}{path}");
    mount_point(&target, false).map_err(|e| format!("mount {path}: {e}"))?;
    mount_bind(path, target.as_str()).map_err(|e| format!("mount {path}: {e}"))
}

/// Shows a unix socket of the host at the same path under root. A path-named socket is reached through the filesystem,
/// so it works across network namespaces: it is the one way out of a jail.
fn bind_socket(root: &str, path: &str) -> Result<(), String> {
    let info = std::fs::metadata(path).map_err(|e| format!("socket {path}: {e}"))?;
    if !info.file_type().is_socket() {
        return Err(format!("socket {path}: not a unix socket"));
    }
    let target = format!("{root}{path}");
    mount_point(&target, false).map_err(|e| format!("socket {path}: {e}"))?;
    mount_bind(path, target.as_str()).map_err(|e| format!("socket {path}: {e}"))
}

/// Makes the path under root (root "" is the host) read as empty: a directory becomes an unreadable empty one, a file
/// becomes /dev/null. A path that is not there hides nothing.
fn hide(root: &str, path: &str) -> Result<(), String> {
    let target = format!("{root}{path}");
    let info = match std::fs::metadata(&target) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        other => other.map_err(|e| format!("hide {path}: {e}"))?,
    };
    let hidden = if info.is_dir() {
        let flags = MountFlags::RDONLY | MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC;
        mount("tmpfs", target.as_str(), "tmpfs", flags, c"size=4k,mode=0000")
    } else {
        mount_bind("/dev/null", target.as_str())
    };
    hidden.map_err(|e| format!("hide {path}: {e}"))
}

/// Makes the place a mount goes: the directory, or an empty file for a file.
fn mount_point(target: &str, dir: bool) -> std::io::Result<()> {
    if dir {
        return std::fs::DirBuilder::new().recursive(true).create(target);
    }
    if let Some(parent) = Path::new(target).parent() {
        std::fs::DirBuilder::new().recursive(true).create(parent)?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(target)
        .map(drop)
}

/// The flags of the mount at path that a bind mount of it keeps and a remount may not drop: the kernel refuses a remount
/// that clears a flag locked by a more privileged namespace. statfs names them ST_*, mount MS_*.
fn inherited_flags(path: &str) -> Result<MountFlags, String> {
    let st = statfs(path).map_err(|e| format!("statfs {path}: {e}"))?;
    const ST: [(u64, MountFlags); 6] = [
        (2, MountFlags::NOSUID),
        (4, MountFlags::NODEV),
        (8, MountFlags::NOEXEC),
        (1024, MountFlags::NOATIME),
        (2048, MountFlags::NODIRATIME),
        (4096, MountFlags::RELATIME),
    ];
    let have = u64::try_from(st.f_flags).unwrap_or(0);
    Ok(ST
        .iter()
        .filter(|(st, _)| have & st != 0)
        .fold(MountFlags::empty(), |acc, (_, m)| acc | *m))
}

/// Makes root the process's root and drops the old one. pivot_root(".", ".") stacks the old root on the new one at the
/// same place, which a detached unmount then removes: it needs no directory to move the old root to.
fn pivot(root: &str) -> Result<(), String> {
    let flags = MountFlags::BIND | MountFlags::RDONLY | MountFlags::NOSUID | MountFlags::NODEV;
    mount_remount(root, flags, "").map_err(|e| format!("make the root read-only: {e}"))?;
    chdir(root).map_err(|e| format!("enter the new root: {e}"))?;
    pivot_root(".", ".").map_err(|e| format!("pivot_root: {e}"))?;
    unmount(".", UnmountFlags::DETACH).map_err(|e| format!("drop the old root: {e}"))
}

/// Brings the namespace's own loopback up: an RTM_NEWLINK request over route netlink that sets IFF_UP on lo, and its
/// acknowledgement. The SIOCSIFFLAGS ioctl would make the same change, but no crate wraps it safely.
pub(crate) fn loopback_up() -> Result<(), String> {
    use rustix::net::netlink::SocketAddrNetlink;
    use rustix::net::{AddressFamily, RecvFlags, SendFlags, SocketType, recv, sendto, socket};

    // Protocol 0 is NETLINK_ROUTE.
    let fd = socket(AddressFamily::NETLINK, SocketType::RAW, None).map_err(|e| format!("loopback: {e}"))?;
    let index = rustix::net::netdevice::name_to_index(&fd, "lo").map_err(|e| format!("loopback: {e}"))?;
    const RTM_NEWLINK: u16 = 16;
    const NLM_F_REQUEST_ACK: u16 = 1 | 4;
    const IFF_UP: u32 = 1;
    // struct nlmsghdr (len, type, flags, seq, pid), then struct ifinfomsg (family, pad, type, index, flags, change).
    let mut msg = Vec::with_capacity(32);
    msg.extend_from_slice(&32u32.to_ne_bytes());
    msg.extend_from_slice(&RTM_NEWLINK.to_ne_bytes());
    msg.extend_from_slice(&NLM_F_REQUEST_ACK.to_ne_bytes());
    msg.extend_from_slice(&1u32.to_ne_bytes());
    msg.extend_from_slice(&0u32.to_ne_bytes());
    msg.extend_from_slice(&[0, 0, 0, 0]);
    msg.extend_from_slice(&index.to_ne_bytes());
    msg.extend_from_slice(&IFF_UP.to_ne_bytes());
    msg.extend_from_slice(&IFF_UP.to_ne_bytes());
    sendto(&fd, &msg, SendFlags::empty(), &SocketAddrNetlink::new(0, 0)).map_err(|e| format!("loopback up: {e}"))?;
    // The acknowledgement is an NLMSG_ERROR whose error, after the 16-byte header, is 0 or a negative errno.
    let mut ack = [0u8; 64];
    let (n, _) = recv(&fd, &mut ack, RecvFlags::empty()).map_err(|e| format!("loopback up: {e}"))?;
    let errno = ack
        .get(16..20)
        .filter(|_| n >= 20)
        .and_then(|b| b.try_into().ok())
        .map(i32::from_ne_bytes)
        .ok_or("loopback up: a short acknowledgement")?;
    if errno != 0 {
        return Err(format!(
            "loopback up: {}",
            std::io::Error::from_raw_os_error(errno.saturating_neg())
        ));
    }
    Ok(())
}
