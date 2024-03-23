#![no_std]
#![feature(allocator_api)]
#![allow(missing_docs)]

#[cfg(test)]
extern crate std;

extern crate alloc;
extern crate libc;

const QUEUE_SIZE: usize = 1024;
const MAX_BATCH_SIZE: usize = 32;

pub mod queue;
mod shmem;
pub mod spsc;

use alloc::string::String;
use shmem::{exists, ShmemAllocator};

#[cfg(feature = "spsc")]
use alloc::vec::Vec;
#[cfg(feature = "mpsc")]
pub use queue::Queue;
#[cfg(feature = "spsc")]
pub use spsc::Queue;

#[repr(transparent)]
pub struct Sender<'a, T>(Queue<'a, T>);

unsafe impl<'a, T: Send> Send for Sender<'a, T> {}
unsafe impl<'a, T: Sync> Sync for Sender<'a, T> {}

impl<'a, T: Send + Clone> Clone for Sender<'a, T> {
    fn clone(&self) -> Sender<'a, T> {
        Sender(self.0.clone())
    }
}

impl<'a, T: Send + Clone> Sender<'a, T> {
    pub fn new(name: &str) -> Sender<'a, T> {
        Sender(
            Queue::<T>::with_capacity_in(
                !exists(name),
                QUEUE_SIZE,
                ShmemAllocator(String::from(name)),
            )
            .unwrap(),
        )
    }

    pub fn send(&self, data: T) -> bool {
        while self.0.enqueue(data.clone()).is_err() {}
        true
    }

    pub fn try_send(&self, data: T) -> bool {
        self.0.enqueue(data).is_ok()
    }

    #[cfg(feature = "spsc")]
    pub fn send_batch(&self, data: &mut Vec<T>) -> bool {
        while self.0.enqueue_batch(data).is_err() {}
        true
    }
}

#[repr(transparent)]
pub struct Receiver<'a, T>(Queue<'a, T>);

unsafe impl<'a, T: Send> Send for Receiver<'a, T> {}
unsafe impl<'a, T: Sync> Sync for Receiver<'a, T> {}

impl<'a, T: Send + Clone> Clone for Receiver<'a, T> {
    fn clone(&self) -> Receiver<'a, T> {
        Receiver(self.0.clone())
    }
}

impl<'a, T: Send> Receiver<'a, T> {
    pub fn new(name: &str) -> Receiver<'a, T> {
        Receiver(
            Queue::<T>::with_capacity_in(
                !exists(name),
                QUEUE_SIZE,
                ShmemAllocator(String::from(name)),
            )
            .unwrap(),
        )
    }

    pub fn recv(&self) -> T {
        loop {
            if let Some(data) = self.0.dequeue() {
                return data;
            }
        }
    }

    #[cfg(feature = "spsc")]
    pub fn try_recv_batch(&self, batch: Vec<Option<T>>) -> Vec<Option<T>> {
        self.0.dequeue_batch(batch)
    }

    pub fn try_recv(&self) -> Option<T> {
        self.0.dequeue()
    }
}
