use crate::sshd::*;
use portable_pty::{MasterPty, PtySize};
use rstest::*;
use std::io::Read;
use std::time::{Duration, Instant};
use wezterm_ssh::Config;

/// Sockets currently open in `pid`.
///
/// ssh-agent accepts one socket per client and does not close it; the client
/// must. A forwarded `auth-agent@openssh.com` channel that is never reaped
/// shows up here as a permanent extra socket.
fn socket_fd_count(pid: u32) -> usize {
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
            return 0;
        };
        entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                std::fs::read_link(entry.path())
                    .ok()
                    .is_some_and(|target| target.to_string_lossy().starts_with("socket:"))
            })
            .count()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        0
    }
}

/// Socket fds held by `root` and every descendant. The remote sshd session
/// process (a child of the listener) keeps one fd per open agent channel.
fn process_tree_socket_fds(root: u32) -> usize {
    #[cfg(target_os = "linux")]
    {
        fn walk(pid: u32, total: &mut usize) {
            *total += socket_fd_count(pid);
            let children = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children"))
                .unwrap_or_default();
            for part in children.split_whitespace() {
                if let Ok(child) = part.parse::<u32>() {
                    walk(child, total);
                }
            }
        }
        let mut total = 0;
        walk(root, &mut total);
        total
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = root;
        0
    }
}

/// Poll until `probe` returns a value `<= limit`, or `timeout` elapses.
fn wait_until_at_most(limit: usize, timeout: Duration, mut probe: impl FnMut() -> usize) -> usize {
    let start = Instant::now();
    loop {
        let count = probe();
        if count <= limit || start.elapsed() >= timeout {
            return count;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Lowest value of `probe` over `window`. A connection that is closed during
/// the window is not kept in the baseline.
fn min_over(window: Duration, mut probe: impl FnMut() -> usize) -> usize {
    let start = Instant::now();
    let mut best = probe();
    while start.elapsed() < window {
        std::thread::sleep(Duration::from_millis(50));
        best = best.min(probe());
    }
    best
}

#[fixture]
async fn session_with_agent_forward(
    #[future]
    #[with({ let mut config = Config::new(); config.set_option("forwardagent", "yes"); config })]
    session: SessionWithSshd,
) -> SessionWithSshd {
    session.await
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
#[cfg_attr(not(feature = "libssh-rs"), ignore)]
fn ssh_add_should_be_able_to_list_identities_with_agent_forward(
    #[future] session_with_agent_forward: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session_with_agent_forward.await;

        let (pty, _child_process) = session
            .request_pty("dumb", PtySize::default(), Some("ssh-add -l"), None)
            .await
            .unwrap();
        let mut reader = pty.try_clone_reader().unwrap();
        let mut output: String = String::new();
        reader.read_to_string(&mut output).unwrap();
        assert_eq!(output, "The agent has no identities.\r\n");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
#[cfg_attr(not(feature = "libssh-rs"), ignore)]
fn no_agent_forward_should_happen_when_disabled(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let (pty, _child_process) = session
            .request_pty("dumb", PtySize::default(), Some("ssh-add -l"), None)
            .await
            .unwrap();
        let mut reader = pty.try_clone_reader().unwrap();
        let mut output: String = String::new();
        reader.read_to_string(&mut output).unwrap();
        assert_eq!(
            output,
            "Could not open a connection to your authentication agent.\r\n"
        );
    })
}

/// Each `ssh-add` opens an `auth-agent@openssh.com` channel proxied to the
/// local agent. The agent keeps that socket open for the life of the
/// connection, so WezTerm must close the channel (CHANNEL_CLOSE) when the
/// remote client is done. Otherwise every use permanently leaks one local
/// agent connection and one fd in the remote sshd session.
#[rstest]
#[cfg_attr(not(target_os = "linux"), ignore)]
#[cfg_attr(not(feature = "libssh-rs"), ignore)]
fn forwarded_agent_channel_is_closed_after_each_use(
    #[future] session_with_agent_forward: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session_with_agent_forward.await;
        let agent_pid = session.agent_pid();
        let sshd_pid = session.sshd_pid();

        // One use first. sshd keeps the forwarded SSH_AUTH_SOCK listener for
        // the life of the session; that fd is not a per-request leak. The
        // baseline is taken after it exists and the proxied connection should
        // already have been closed.
        run_ssh_add(&session).await;
        let settle = Duration::from_secs(3);
        let agent_before = min_over(Duration::from_secs(1), || socket_fd_count(agent_pid));
        let remote_before = min_over(Duration::from_millis(200), || {
            process_tree_socket_fds(sshd_pid)
        });

        // Several further uses. A leak is one socket per use.
        const USES: usize = 3;
        for _ in 0..USES {
            run_ssh_add(&session).await;
        }

        let agent_after = wait_until_at_most(agent_before, settle, || socket_fd_count(agent_pid));
        let remote_after =
            wait_until_at_most(remote_before, settle, || process_tree_socket_fds(sshd_pid));

        assert_eq!(
            agent_after,
            agent_before,
            "local ssh-agent leaked {} connection(s) across {USES} forwarded agent requests \
             (before {agent_before}, after {agent_after})",
            agent_after.saturating_sub(agent_before),
        );
        assert_eq!(
            remote_after,
            remote_before,
            "remote sshd leaked {} socket(s) across {USES} forwarded agent requests \
             (before {remote_before}, after {remote_after})",
            remote_after.saturating_sub(remote_before),
        );
    })
}

async fn run_ssh_add(session: &wezterm_ssh::Session) {
    let (pty, _child) = session
        .request_pty("dumb", PtySize::default(), Some("ssh-add -l"), None)
        .await
        .unwrap();
    let mut reader = pty.try_clone_reader().unwrap();
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();
    assert_eq!(output, "The agent has no identities.\r\n");
}
