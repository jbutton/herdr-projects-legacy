//! What macOS says about this user's processes, from the system itself (libproc and sysctl) where Linux reads /proc:
//! every process of theirs, the program it runs, the folder it runs in and the environment it was started with.
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

/// This user's processes now.
pub fn mine() -> Vec<i32> {
    let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if n <= 0 { return vec![]; }
    let mut all = vec![0i32; n as usize + 64]; // room for those started in between
    let got = unsafe { libc::proc_listallpids(all.as_mut_ptr() as *mut libc::c_void, (all.len() * std::mem::size_of::<i32>()) as i32) };
    all.truncate(got.max(0) as usize);
    let me = crate::agents::scratch::uid();
    all.into_iter().filter(|p| *p > 0 && uid_of(*p) == Some(me)).collect()
}
/// Whose a process is, where the system tells.
fn uid_of(pid: i32) -> Option<u32> {
    let mut i: libc::proc_bsdshortinfo = unsafe { std::mem::zeroed() };
    let n = std::mem::size_of::<libc::proc_bsdshortinfo>() as i32;
    (unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDT_SHORTBSDINFO, 0, &mut i as *mut _ as *mut libc::c_void, n) } == n).then_some(i.pbsi_uid)
}
/// The program a process runs, as the system found it when it started it.
pub fn program_of(pid: i32) -> Option<PathBuf> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut libc::c_void, buf.len() as u32) };
    if n <= 0 { return None; }
    buf.truncate(n as usize);
    Some(PathBuf::from(OsStr::from_bytes(&buf)))
}
/// The folder a process runs in.
pub fn folder_of(pid: i32) -> Option<PathBuf> {
    let mut i: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let n = std::mem::size_of::<libc::proc_vnodepathinfo>() as i32;
    if unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDVNODEPATHINFO, 0, &mut i as *mut _ as *mut libc::c_void, n) } != n { return None; }
    let dir = unsafe { std::ffi::CStr::from_ptr(i.pvi_cdir.vip_path.as_ptr() as *const libc::c_char) };
    Some(PathBuf::from(OsStr::from_bytes(dir.to_bytes()))).filter(|d| !d.as_os_str().is_empty())
}
/// The environment a process was started with, `KEY=value` each ended by a nul, as Linux keeps it in
/// /proc/<pid>/environ. The system gives it for this user's own processes only.
pub fn environ(pid: i32) -> Option<Vec<u8>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: libc::size_t = 0;
    let ask = |mib: &mut [i32; 3], buf: *mut libc::c_void, size: &mut libc::size_t| unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buf, size, std::ptr::null_mut(), 0) } == 0;
    if !ask(&mut mib, std::ptr::null_mut(), &mut size) || size == 0 { return None; }
    let mut buf = vec![0u8; size];
    if !ask(&mut mib, buf.as_mut_ptr() as *mut libc::c_void, &mut size) { return None; }
    buf.truncate(size);
    environ_in(&buf)
}
/// The environment in what the system says of a process's start: how many arguments, the program's path and the nuls
/// after it, the arguments, then the environment up to an empty entry.
fn environ_in(buf: &[u8]) -> Option<Vec<u8>> {
    let argc = i32::from_ne_bytes(buf.get(..4)?.try_into().ok()?);
    let rest = &buf[4..];
    let path_end = rest.iter().position(|b| *b == 0)?;
    let args = path_end + rest[path_end..].iter().position(|b| *b != 0)?;
    let entries = rest[args..].split(|b| *b == 0).skip(argc.max(0) as usize);
    let mut env = vec![];
    for e in entries.take_while(|e| !e.is_empty()) { env.extend_from_slice(e); env.push(0); }
    Some(env)
}
