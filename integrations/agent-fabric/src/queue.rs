//! Bounded FIFO queues: the mechanism behind backpressure.
//!
//! Every queue in the fabric is bounded. `push` returns [`Backpressure`] when full and the
//! caller decides — the queue never drops, never evicts, and never grows. The high-water mark is
//! tracked so memory-bound tests can assert the bound held under load rather than trusting the
//! capacity parameter.

use std::collections::VecDeque;
use std::fmt;

/// The only failure a bounded push can produce, carrying the numbers needed to report it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backpressure {
    pub capacity: usize,
}

impl fmt::Display for Backpressure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "queue at capacity {}", self.capacity)
    }
}

impl std::error::Error for Backpressure {}

#[derive(Debug)]
pub struct BoundedQueue<T> {
    buf: VecDeque<T>,
    cap: usize,
    hwm: usize,
}

impl<T> BoundedQueue<T> {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "a zero-capacity queue cannot make progress");
        Self {
            buf: VecDeque::with_capacity(cap),
            cap,
            hwm: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Highest length ever reached. Monotone; the observable for "the bound was never exceeded".
    pub fn high_water(&self) -> usize {
        self.hwm
    }

    /// Enqueues or reports backpressure. Never blocks, never drops.
    pub fn push(&mut self, item: T) -> Result<(), Backpressure> {
        if self.buf.len() >= self.cap {
            return Err(Backpressure { capacity: self.cap });
        }
        self.buf.push_back(item);
        self.hwm = self.hwm.max(self.buf.len());
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        self.buf.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_queue_reports_backpressure_instead_of_dropping() {
        let mut q: BoundedQueue<u32> = BoundedQueue::new(2);
        assert!(q.push(1).is_ok());
        assert!(q.push(2).is_ok());
        assert_eq!(q.push(3), Err(Backpressure { capacity: 2 }));
        assert_eq!(q.pop(), Some(1));
        assert!(q.push(3).is_ok());
    }

    #[test]
    fn fifo_order_is_preserved_exactly() {
        let mut q: BoundedQueue<usize> = BoundedQueue::new(4);
        for i in 0..4 {
            q.push(i).expect("fits");
        }
        assert_eq!(
            (0..4).map(|_| q.pop().expect("queued")).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
    }

    #[test]
    fn high_water_records_the_peak_but_never_the_capacity_when_unused() {
        let mut q: BoundedQueue<u8> = BoundedQueue::new(10);
        q.push(1).expect("fits");
        q.push(2).expect("fits");
        q.pop();
        assert_eq!(q.high_water(), 2);
        assert_eq!(q.len(), 1);
    }
}
