use std::fmt;
use std::iter::FusedIterator;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use super::{EventSink, EventSinkStream, EventSinkWriter};

/// A blocking event queue with an unbounded size.
///
/// Implements [`EventSink`].
///
/// Note that [`EventSinkStream`] is implemented by
/// [`BlockingEventQueueReader`], created with the
/// [`BlockingEventQueue::into_reader`] method.
pub struct BlockingEventQueue<T> {
    is_open: Arc<AtomicBool>,
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T> BlockingEventQueue<T> {
    /// Creates an open `BlockingEventQueue`.
    pub fn new() -> Self {
        Self::new_with_state(true)
    }

    /// Creates a closed `BlockingEventQueue`.
    pub fn new_closed() -> Self {
        Self::new_with_state(false)
    }

    /// Returns a consumer handle.
    pub fn into_reader(self) -> BlockingEventQueueReader<T> {
        BlockingEventQueueReader {
            is_open: self.is_open,
            receiver: self.receiver,
        }
    }

    /// Creates a new `BlockingEventQueue` in the specified state.
    fn new_with_state(is_open: bool) -> Self {
        let (sender, receiver) = channel();
        Self {
            is_open: Arc::new(AtomicBool::new(is_open)),
            sender,
            receiver,
        }
    }
}

impl<T: Send + 'static> EventSink<T> for BlockingEventQueue<T> {
    type Writer = BlockingEventQueueWriter<T>;

    fn writer(&self) -> Self::Writer {
        BlockingEventQueueWriter {
            is_open: self.is_open.clone(),
            sender: self.sender.clone(),
        }
    }
}

impl<T> Default for BlockingEventQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for BlockingEventQueue<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BlockingEventQueue").finish_non_exhaustive()
    }
}

/// A consumer handle of a `BlockingEventQueue`.
///
/// Implements [`EventSinkStream`]. Calls to the iterator's `next` method are
/// blocking. `None` is returned when all writer handles have been dropped.
pub struct BlockingEventQueueReader<T> {
    is_open: Arc<AtomicBool>,
    receiver: Receiver<T>,
}

impl<T> Iterator for BlockingEventQueueReader<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self.receiver.recv() {
            Ok(event) => Some(event),
            Err(_) => None,
        }
    }
}

impl<T> FusedIterator for BlockingEventQueueReader<T> {}

impl<T: Send + 'static> EventSinkStream for BlockingEventQueueReader<T> {
    fn open(&mut self) {
        self.is_open.store(true, Ordering::Relaxed);
    }

    fn close(&mut self) {
        self.is_open.store(false, Ordering::Relaxed);
    }
}

impl<T> fmt::Debug for BlockingEventQueueReader<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BlockingEventQueueReader")
            .finish_non_exhaustive()
    }
}

/// A producer handle of a `BlockingEventQueue`.
pub struct BlockingEventQueueWriter<T> {
    is_open: Arc<AtomicBool>,
    sender: Sender<T>,
}

impl<T: Send + 'static> EventSinkWriter<T> for BlockingEventQueueWriter<T> {
    /// Pushes an event onto the queue.
    fn write(&self, event: T) {
        if !self.is_open.load(Ordering::Relaxed) {
            return;
        }
        // Ignore sending failure.
        let _ = self.sender.send(event);
    }
}

impl<T> Clone for BlockingEventQueueWriter<T> {
    fn clone(&self) -> Self {
        Self {
            is_open: self.is_open.clone(),
            sender: self.sender.clone(),
        }
    }
}

impl<T> fmt::Debug for BlockingEventQueueWriter<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BlockingEventQueueWriter")
            .finish_non_exhaustive()
    }
}
