//! A test's world binds the fake herdr's socket in a short folder of its own: however long the temporary folder is
//! (macOS gives each user a long one, the board each task a folder in it), the socket's path stays well under what a
//! Unix socket may have, and the world works. Made in /tmp itself (Linux's temporary folder), its own folder stays apart
//! from its sockets' folder.
use crate::common;
use common::*;
use serde_json::json;

#[test]
fn a_worlds_socket_stays_short_however_long_its_folder_is() {
    // A folder far longer than any socket's path may be, in the temporary folder.
    let under = format!("swt-long-{}/{}/{}", std::process::id(), "a".repeat(100), "b".repeat(100));
    let w = world_in(Opts { repo: true, ..Default::default() }, &under);
    assert!(w.base.as_os_str().len() > 2 * SOCKET_PATH_MAX, "{}", w.base.display());
    assert!(!w.sockets.starts_with(&w.base) && w.socket.len() <= SOCKET_PATH_MAX / 2, "at most half of {SOCKET_PATH_MAX}: {}", w.socket);
    assert!(sw::herdr::cli::socket_exists(&w.socket), "herdr's socket is there: {}", w.socket);
    let events = w.sock("events-12345.sock");
    let _l = std::os::unix::net::UnixListener::bind(&events).unwrap();
    assert!(sw::herdr::cli::socket_exists(&events.display().to_string()));
    // The world works in it: a task through its agent, its check and its review.
    to_approval(&w, "Works in a long folder");
    // And leaves nothing behind: its sockets' folder goes with it.
    let (sockets, long) = (w.sockets.clone(), w.base.ancestors().nth(3).unwrap().to_path_buf());
    assert!(long.ends_with(format!("swt-long-{}", std::process::id())), "{}", long.display());
    drop(w);
    assert!(!sockets.exists(), "{}", sockets.display());
    std::fs::remove_dir_all(&long).unwrap();
}

#[test]
fn a_world_made_in_tmp_itself_keeps_its_home_apart_from_its_sockets() {
    // /tmp is the temporary folder on Linux when no TMPDIR names another (GitHub's runners): the world's folder and its
    // sockets' folder are then side by side in it, and neither may be the other.
    let w = world_in(Opts { repo: true, repo_settings: Some(json!({ "check.command": "echo run >> \"$HOME/checks.log\"" })), ..Default::default() }, "/tmp");
    assert!(w.base.parent() == w.sockets.parent() && w.base != w.sockets, "{} {}", w.base.display(), w.sockets.display());
    assert!(w.base.join("home").is_dir(), "its home is there: {}", w.base.display());
    // A check that writes in $HOME passes there.
    to_approval(&w, "Checked in /tmp");
    assert_eq!(read(&w.base.join("home/checks.log")), "run\n");
    let (base, sockets) = (w.base.clone(), w.sockets.clone());
    drop(w);
    assert!(!base.exists() && !sockets.exists(), "{} {}", base.display(), sockets.display());
}
