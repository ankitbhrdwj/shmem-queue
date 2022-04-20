#![no_std]
#![feature(allocator_api)]

#[cfg(test)]
extern crate std;

extern crate alloc;
extern crate libc;

const QUEUE_SIZE: usize = 1024;

mod queue;
mod shmem;

use queue::Queue;

#[repr(transparent)]
pub struct Sender<'a, T>(Queue<'a, T>);

unsafe impl<'a, T: Send> Send for Sender<'a, T> {}
unsafe impl<'a, T: Sync> Sync for Sender<'a, T> {}

impl<'a, T: Send> Sender<'a, T> {
    pub fn new(name: &str) -> Sender<'a, T> {
        Sender(Queue::<T>::new(name).unwrap())
    }

    pub fn send(&self, data: T) -> bool {
        self.0.enqueue(data).is_ok()
    }

    pub fn try_send(&self, data: T) -> bool {
        self.0.enqueue(data).is_ok()
    }
}

#[repr(transparent)]
pub struct Receiver<'a, T>(Queue<'a, T>);

unsafe impl<'a, T: Send> Send for Receiver<'a, T> {}
unsafe impl<'a, T: Sync> Sync for Receiver<'a, T> {}

impl<'a, T: Send> Receiver<'a, T> {
    pub fn new(name: &str) -> Receiver<'a, T> {
        Receiver(Queue::<T>::new(name).unwrap())
    }

    pub fn recv(&self) -> T {
        loop {
            if let Some(data) = self.0.dequeue() {
                return data;
            }
        }
    }

    pub fn try_recv(&self) -> Option<T> {
        self.0.dequeue()
    }
}
