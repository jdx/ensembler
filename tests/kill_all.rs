//! `kill_all` signals every running child in the process, so this lives in its
//! own test binary where no other test's children can be hit.

#![cfg(unix)]

use ensembler::CmdLineRunner;
use nix::sys::signal::Signal;
use std::time::{Duration, Instant};

fn is_alive(pid: &str) -> bool {
    let out = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .unwrap();
    let stat = String::from_utf8(out.stdout).unwrap();
    let stat = stat.trim();
    // An unreaped zombie is dead for our purposes.
    !stat.is_empty() && !stat.starts_with('Z')
}

async fn wait_for_pid(path: &std::path::Path) -> String {
    let start = Instant::now();
    loop {
        if let Ok(pid) = std::fs::read_to_string(path) {
            // Written with a trailing newline, so a partial write isn't mistaken for a pid.
            if pid.ends_with('\n') {
                return pid.trim().to_string();
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "child never started"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn test_kill_all_reaches_interactive_children_and_descendants() {
    // Unique per run, so a stale file from a reused PID can't satisfy the waits below.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("ensembler-kill-all-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let grandchild_file = dir.join("grandchild");
    let interactive_file = dir.join("interactive");

    // Leads its own process group, with a descendant that only a group signal reaches.
    let grouped = tokio::spawn({
        let grandchild_file = grandchild_file.clone();
        async move {
            CmdLineRunner::new("sh")
                .args(["-c", r#"sleep 30 & echo $! > "$1"; wait"#, "sh"])
                .arg(grandchild_file)
                .execute()
                .await
        }
    });
    // Shares our process group, so it must be signaled by pid.
    let interactive = tokio::spawn({
        let interactive_file = interactive_file.clone();
        async move {
            CmdLineRunner::new("sh")
                .args(["-c", r#"echo $$ > "$1"; exec sleep 30"#, "sh"])
                .arg(interactive_file)
                .interactive(true)
                .execute()
                .await
        }
    });

    // Both children report in once running. The runner registers a child right
    // after spawning it, with no await in between, long before the child's shell
    // gets to write its file.
    let grandchild = wait_for_pid(&grandchild_file).await;
    wait_for_pid(&interactive_file).await;
    assert!(is_alive(&grandchild));

    CmdLineRunner::kill_all(Signal::SIGTERM);

    let start = Instant::now();
    let (grouped, interactive) = tokio::join!(grouped, interactive);
    assert!(start.elapsed() < Duration::from_secs(10));
    assert!(grouped.unwrap().is_err());
    assert!(interactive.unwrap().is_err());

    let start = Instant::now();
    while is_alive(&grandchild) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "descendant survived"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let _ = std::fs::remove_dir_all(&dir);
}
