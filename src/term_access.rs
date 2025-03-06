use std::borrow::Borrow;
use std::io;
use std::io::{Error, ErrorKind};
use console::{Key, Term};
use crate::term_access::InterruptableResult::{AsRequested, IOError};

pub enum InterruptableResult<T> {
    AsRequested(T),
    WriteLine(String),
    IOError(Option<io::Error>),
    AccessRevoked
}

impl<T> InterruptableResult<T> {
    pub fn try_into_error(&mut self) -> io::Result<()> {
        match self {
            IOError(err) if err.is_some() => Err(err.take().unwrap()),
            InterruptableResult::AccessRevoked => Err(Error::new(ErrorKind::BrokenPipe, "Term access was revoked, possibly due to channel being closed")),
            _ => Ok(())
        }
    }
}

pub trait TermAccess {

    fn term(&self) -> Term;
    async fn read_key_async(&mut self) -> InterruptableResult<Key>;
    fn read_key_blocking(&mut self) -> InterruptableResult<Key>;

}
impl TermAccess for &Term {

    fn term(&self) -> Term {
        (**self).clone()
    }

    async fn read_key_async(&mut self) -> InterruptableResult<Key> {
        self.read_key_blocking()
    }

    fn read_key_blocking(&mut self) -> InterruptableResult<Key> {
        match self.read_key() {
            Ok(k) => AsRequested(k),
            Err(e) => IOError(Some(e)),
        }
    }
}


#[cfg(feature = "tokio")]
pub use tokio::{TokioChannelConsoleAccess};

#[cfg(feature = "tokio")]
mod tokio {
    use console::{Key, Term};
    use tokio::sync::mpsc::{Receiver, UnboundedReceiver};
    use tokio::select;
    use crate::term_access::{TermAccess, InterruptableResult};
    use crate::term_access::InterruptableResult::{AccessRevoked, AsRequested, WriteLine};

    pub struct TokioChannelConsoleAccess {
        pub term: Term,
        pub key_receiver: Receiver<Key>,
        pub write_line_receiver: UnboundedReceiver<String>
    }

    impl TermAccess for TokioChannelConsoleAccess {
        fn term(&self) -> Term {
            self.term.clone()
        }

        async fn read_key_async(&mut self) -> InterruptableResult<Key> {
            select! {
                Some(key) = self.key_receiver.recv() => AsRequested(key),
                Some(line) = self.write_line_receiver.recv() => WriteLine(line),
                else => AccessRevoked
            }
        }

        fn read_key_blocking(&mut self) -> InterruptableResult<Key> {
            futures::executor::block_on(self.read_key_async())
        }
    }
}

