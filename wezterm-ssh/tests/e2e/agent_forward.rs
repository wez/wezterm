use crate::sshd::*;
use portable_pty::{MasterPty, PtySize};
use rstest::*;
use std::io::Read;
use wezterm_ssh::Config;

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
