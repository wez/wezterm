use crate::channelwrap::ChannelWrap;
use crate::sftpwrap::SftpWrap;
use filedescriptor::{AsRawSocketDescriptor, SocketDescriptor, POLLIN, POLLOUT};

#[cfg(feature = "ssh2")]
pub(crate) struct Ssh2Session {
    pub sess: ssh2::Session,
    pub sftp: Option<SftpWrap>,
}

#[cfg(feature = "libssh-rs")]
pub(crate) struct LibSshSession {
    pub sess: libssh_rs::Session,
    pub sftp: Option<SftpWrap>,
}

pub(crate) enum SessionWrap {
    #[cfg(feature = "ssh2")]
    Ssh2(Ssh2Session),

    #[cfg(feature = "libssh-rs")]
    LibSsh(LibSshSession),
}

impl SessionWrap {
    #[cfg(feature = "ssh2")]
    pub fn with_ssh2(sess: ssh2::Session) -> Self {
        Self::Ssh2(Ssh2Session { sess, sftp: None })
    }

    #[cfg(feature = "libssh-rs")]
    pub fn with_libssh(sess: libssh_rs::Session) -> Self {
        Self::LibSsh(LibSshSession { sess, sftp: None })
    }

    pub fn set_blocking(&mut self, blocking: bool) {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(sess) => sess.sess.set_blocking(blocking),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(sess) => sess.sess.set_blocking(blocking),
        }
    }

    pub fn get_poll_flags(&self) -> i16 {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(sess) => match sess.sess.block_directions() {
                ssh2::BlockDirections::None => 0,
                ssh2::BlockDirections::Inbound => POLLIN,
                ssh2::BlockDirections::Outbound => POLLOUT,
                ssh2::BlockDirections::Both => POLLIN | POLLOUT,
            },

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(sess) => {
                let (read, write) = sess.sess.get_poll_state();
                match (read, write) {
                    (false, false) => 0,
                    (true, false) => POLLIN,
                    (false, true) => POLLOUT,
                    (true, true) => POLLIN | POLLOUT,
                }
            }
        }
    }

    pub fn as_socket_descriptor(&self) -> SocketDescriptor {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(sess) => sess.sess.as_socket_descriptor(),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(sess) => sess.sess.as_socket_descriptor(),
        }
    }

    pub fn open_session(&self) -> anyhow::Result<ChannelWrap> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(sess) => {
                let channel = sess.sess.channel_session()?;
                Ok(ChannelWrap::Ssh2(channel))
            }

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(sess) => {
                let channel = sess.sess.new_channel()?;
                channel.open_session()?;
                Ok(ChannelWrap::LibSsh(channel))
            }
        }
    }

    pub fn accept_agent_forward(&mut self) -> Option<ChannelWrap> {
        match self {
            // Unimplemented for now, an error message was printed earlier when the user tries to
            // enable agent forwarding so just return nothing here.
            #[cfg(feature = "ssh2")]
            Self::Ssh2(_sess) => None,

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(sess) => sess.sess.accept_agent_forward().map(ChannelWrap::LibSsh),
        }
    }
}
