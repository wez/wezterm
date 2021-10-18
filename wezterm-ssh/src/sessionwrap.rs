use crate::channelwrap::ChannelWrap;
use filedescriptor::{AsRawSocketDescriptor, SocketDescriptor, POLLIN, POLLOUT};
use libssh_rs as libssh;
use ssh2::BlockDirections;

pub(crate) struct Ssh2Session {
    pub sess: ssh2::Session,
    pub sftp: Option<ssh2::Sftp>,
}

pub(crate) enum SessionWrap {
    Ssh2(Ssh2Session),
    LibSsh(libssh::Session),
}

impl SessionWrap {
    pub fn with_ssh2(sess: ssh2::Session) -> Self {
        Self::Ssh2(Ssh2Session { sess, sftp: None })
    }

    pub fn with_libssh(sess: libssh::Session) -> Self {
        Self::LibSsh(sess)
    }

    pub fn set_blocking(&mut self, blocking: bool) {
        match self {
            Self::Ssh2(sess) => sess.sess.set_blocking(blocking),
            Self::LibSsh(sess) => sess.set_blocking(blocking),
        }
    }

    pub fn get_poll_flags(&self) -> i16 {
        match self {
            Self::Ssh2(sess) => match sess.sess.block_directions() {
                BlockDirections::None => 0,
                BlockDirections::Inbound => POLLIN,
                BlockDirections::Outbound => POLLOUT,
                BlockDirections::Both => POLLIN | POLLOUT,
            },
            Self::LibSsh(sess) => {
                let (read, write) = sess.get_poll_state();
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
            Self::Ssh2(sess) => sess.sess.as_socket_descriptor(),
            Self::LibSsh(sess) => sess.as_socket_descriptor(),
        }
    }

    pub fn open_session(&self) -> anyhow::Result<ChannelWrap> {
        match self {
            Self::Ssh2(sess) => {
                let channel = sess.sess.channel_session()?;
                Ok(ChannelWrap::Ssh2(channel))
            }
            Self::LibSsh(sess) => {
                let channel = sess.new_channel()?;
                channel.open_session()?;
                Ok(ChannelWrap::LibSsh(channel))
            }
        }
    }
}
