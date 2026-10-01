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

#[tokio::test]
async fn test_kill_all_reaches_interactive_children_and_descendants() {
    let dir = std::env::temp_dir().join(format!("ensembler-kill-all-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pid_file = dir.join("grandchild");

    // Leads its own process group, with a descendant that only a group signal reaches.
    let grouped = tokio::spawn({
        let pid_file = pid_file.clone();
        async move {
            CmdLineRunner::new("sh")
                .args(["-c", r#"sleep 30 & echo $! > "$1"; wait"#, "sh"])
                .arg(pid_file)
                .execute()
                .await
        }
    });
    // Shares our process group, so it must be signaled by pid.
    let interactive = tokio::spawn(async {
        CmdLineRunner::new("sleep")
            .arg("30")
            .interactive(true)
            .execute()
            .await
    });

    let start = Instant::now();
    while !pid_file.exists()
        || std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .is_empty()
    {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "child never started"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Give the interactive child time to register as running.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let grandchild = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .to_string();
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
}
