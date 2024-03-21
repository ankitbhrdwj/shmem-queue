use crate::{MAX_BATCH_SIZE, QUEUE_SIZE};
use alloc::alloc::alloc;
use alloc::alloc::{Allocator, Layout};
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::mem::{align_of, size_of};
use core::sync::atomic::{AtomicUsize, Ordering};

type QueueEntry<T> = [UnsafeCell<Option<T>>; QUEUE_SIZE];

#[derive(Debug, Clone)]
pub struct Queue<'a, T> {
    log: &'a QueueEntry<T>,
    head: *const AtomicUsize,
    tail: *const AtomicUsize,
}

impl<'a, T> Default for Queue<'a, T> {
    fn default() -> Self {
        let buf_size = size_of::<QueueEntry<T>>() + size_of::<AtomicUsize>() * 2;
        let mem = unsafe {
            alloc(
                Layout::from_size_align(buf_size, align_of::<Queue<T>>())
                    .expect("Alignment error while allocating the Queue!"),
            )
        };
        if mem.is_null() {
            panic!("Failed to allocate memory for the Queue!");
        }
        Queue::new(mem as *mut u8)
    }
}

/// This is just to make the compiler happy
/// when using queue with multiple threads.
unsafe impl<'a, T> Send for Queue<'a, T> {}
unsafe impl<'a, T> Sync for Queue<'a, T> {}

impl<'a, T> Queue<'a, T> {
    #[allow(clippy::result_unit_err)]
    pub fn with_capacity_in<A: Allocator>(
        _create: bool,
        _capacity: usize,
        _allocator: A,
    ) -> Result<Queue<'a, T>, ()> {
        assert!(_capacity == QUEUE_SIZE);
        let buf_size = size_of::<QueueEntry<T>>() + size_of::<AtomicUsize>() * 2;
        let mem = _allocator
            .allocate(Layout::from_size_align(buf_size, align_of::<Queue<T>>()).unwrap())
            .expect("Allocation failed");
        let mem = mem.as_ptr() as *mut u8;
        Ok(Self::new(mem))
    }

    #[allow(clippy::missing_safety_doc, clippy::not_unsafe_ptr_arg_deref)]
    pub fn new(mem: *mut u8) -> Queue<'a, T> {
        let log = unsafe { &mut *(mem as *mut QueueEntry<T>) };
        let head = unsafe { mem.add(size_of::<QueueEntry<T>>()) } as *mut AtomicUsize;
        let tail: *mut AtomicUsize =
            unsafe { mem.add(size_of::<QueueEntry<T>>() + size_of::<AtomicUsize>()) }
                as *mut AtomicUsize;
        Queue { log, head, tail }
    }

    fn head(&self) -> usize {
        unsafe { (*self.head).load(Ordering::Acquire) }
    }

    fn tail(&self) -> usize {
        unsafe { (*self.tail).load(Ordering::Acquire) }
    }

    pub fn enqueue(&self, value: T) -> Result<(), T> {
        if self.head() == self.tail() + QUEUE_SIZE - 1 {
            return Err(value);
        }
        log::debug!("head: {}, tail: {}", self.head(), self.tail());

        unsafe {
            *self.log.get_unchecked(self.head() % QUEUE_SIZE).get() = Some(value);
            (*self.head).fetch_add(1, Ordering::Release);
        }
        Ok(())
    }

    #[allow(clippy::result_unit_err)]
    pub fn enqueue_batch(&self, values: Vec<T>) -> Result<(), ()> {
        if self.head() + values.len() > self.tail() + QUEUE_SIZE {
            return Err(());
        }
        log::debug!("head: {}, tail: {}", self.head(), self.tail());

        let batch_len = values.len();
        unsafe {
            if let Some(value) = values.into_iter().next() {
                *self.log.get_unchecked(self.head() % QUEUE_SIZE).get() = Some(value);
            }

            (*self.head).fetch_add(batch_len, Ordering::Release);
        }
        Ok(())
    }

    pub fn dequeue(&self) -> Option<T> {
        if self.head() == self.tail() {
            return None;
        }
        log::debug!("head: {}, tail: {}", self.head(), self.tail());

        unsafe {
            let value = (*self.log[self.tail() % QUEUE_SIZE].get()).take();
            (*self.tail).fetch_add(1, Ordering::Release);
            value
        }
    }

    pub fn dequeue_batch(&self) -> Vec<Option<T>> {
        let mut batch = Vec::with_capacity(MAX_BATCH_SIZE);
        let mut tail = self.tail();
        let head = self.head();
        while tail < head && batch.len() < MAX_BATCH_SIZE {
            batch.push(unsafe {
                let value = (*self.log[tail % QUEUE_SIZE].get()).take();
                tail += 1;
                value
            });
        }
        unsafe {
            (*self.tail).store(tail, Ordering::Release);
        }
        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shmem;

    fn mem() -> *mut u8 {
        let size = size_of::<QueueEntry<i32>>() + size_of::<AtomicUsize>() * 2;
        let mem = shmem::create_shm("test", size);
        mem as *mut u8
    }

    #[test]
    fn test_default_initialization() {
        std::fs::remove_file("/dev/shm/test").ok();
        let queue = Queue::<i32>::new(mem());
        assert!(queue.log.len() == QUEUE_SIZE);
        assert_eq!(queue.head(), 0);
        assert_eq!(queue.tail(), 0);
        for i in 0..QUEUE_SIZE {
            let ele = unsafe { queue.log[i].get().as_ref().unwrap() };
            assert!(ele.is_none());
        }
    }

    #[test]
    fn test_enqueue() {
        std::fs::remove_file("/dev/shm/test").ok();
        let queue = Queue::<i32>::new(mem());
        assert!(queue.enqueue(1).is_ok());
        assert_eq!(queue.head(), 1);
    }

    #[test]
    fn test_dequeue() {
        std::fs::remove_file("/dev/shm/test").ok();
        let queue = Queue::<i32>::new(mem());
        assert!(queue.enqueue(1).is_ok());
        assert_eq!(queue.head(), 1);
        assert_eq!(queue.tail(), 0);

        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.head(), 1);
        assert_eq!(queue.tail(), 1);
    }

    #[test]
    fn test_equeue_full() {
        std::fs::remove_file("/dev/shm/test").ok();
        let queue = Queue::<i32>::new(mem());
        for i in 0..QUEUE_SIZE - 1 {
            assert!(queue.enqueue(i as i32).is_ok());
        }
        assert!(queue.tail() == 0);
        assert!(queue.head() == QUEUE_SIZE - 1);
        assert!(!queue.enqueue(QUEUE_SIZE as i32).is_ok());
    }

    #[test]
    fn test_dequeue_empty() {
        std::fs::remove_file("/dev/shm/test").ok();
        let queue = Queue::<i32>::new(mem());
        assert_eq!(queue.dequeue(), None);
    }

    #[test]
    fn test_two_clients() {
        std::fs::remove_file("/dev/shm/test").ok();
        let producer = Queue::<i32>::new(mem());
        let consumer = Queue::<i32>::new(mem());

        assert!(producer.enqueue(1).is_ok());
        assert_eq!(producer.head(), 1);
        assert_eq!(producer.tail(), 0);

        assert_eq!(consumer.dequeue(), Some(1));
        assert_eq!(consumer.head(), 1);
        assert_eq!(consumer.tail(), 1);
    }

    #[test]
    fn test_parallel_client() {
        std::fs::remove_file("/dev/shm/test").ok();
        let producer = Queue::<i32>::new(mem());
        let consumer = Queue::<i32>::new(mem());
        let num_iterations = 10 * QUEUE_SIZE;

        let producer_thread = std::thread::spawn(move || {
            for i in 0..num_iterations {
                while producer.enqueue(i as i32).is_err() {}
            }
        });

        let consumer_thread = std::thread::spawn(move || {
            for i in 0..num_iterations {
                loop {
                    if let Some(value) = consumer.dequeue() {
                        assert_eq!(value, i as i32);
                        break;
                    }
                }
            }
        });

        producer_thread.join().unwrap();
        consumer_thread.join().unwrap();
    }
}
