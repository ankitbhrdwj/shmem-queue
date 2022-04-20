// Copyright © 2021 VMware, Inc. All Rights Reserved.
// Copyright © 2010-2011 Dmitry Vyukov. All rights reserved.
// SPDX-License-Identifier: BSD-2-Clause

// http://www.1024cores.net/home/lock-free-algorithms/queues/bounded-mpmc-queue
// This queue is copy pasted from old rust stdlib.

// Taken from - https://github.com/brayniac/mpmc/blob/master/src/lib.rs

#![allow(warnings)]

use alloc::alloc::{alloc, Layout};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::{align_of, size_of};
use core::slice::from_raw_parts_mut;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use crossbeam_utils::CachePadded;

use crate::QUEUE_SIZE;

struct Node<T> {
    sequence: AtomicUsize,
    value: Option<T>,
}

unsafe impl<T: Send> Send for Node<T> {}
unsafe impl<T: Sync> Sync for Node<T> {}

struct State<'a, T> {
    buffer: &'a [UnsafeCell<Node<T>>],
    mask: usize,
    enqueue_pos: CachePadded<AtomicUsize>,
    dequeue_pos: CachePadded<AtomicUsize>,
}

unsafe impl<'a, T: Send> Send for State<'a, T> {}
unsafe impl<'a, T: Sync> Sync for State<'a, T> {}

pub struct Queue<'a, T> {
    state: Arc<State<'a, T>>,
    phantom: PhantomData<&'a T>,
}

impl<'a, T: Send> State<'a, T> {
    fn with_capacity(capacity: usize) -> Result<State<'a, T>, ()> {
        let capacity = if capacity < 2 || (capacity & (capacity - 1)) != 0 {
            if capacity < 2 {
                2
            } else {
                // use next power of 2 as capacity
                capacity.next_power_of_two()
            }
        } else {
            capacity
        };

        let b = capacity * size_of::<UnsafeCell<Node<T>>>();
        let mem = unsafe {
            alloc(
                Layout::from_size_align(b, align_of::<UnsafeCell<Node<T>>>())
                    .expect("Alignment error while allocating the shared log!"),
            )
        };
        if mem.is_null() {
            panic!("Failed to allocate memory for the shared log!");
        }
        let buffer = unsafe { from_raw_parts_mut(mem as *mut UnsafeCell<Node<T>>, capacity) };

        for (i, e) in buffer.iter_mut().enumerate() {
            unsafe {
                ::core::ptr::write(
                    e,
                    UnsafeCell::new(Node {
                        sequence: AtomicUsize::new(i),
                        value: None,
                    }),
                );
            }
        }

        Ok(State {
            buffer,
            mask: capacity - 1,
            enqueue_pos: CachePadded::new(AtomicUsize::new(0)),
            dequeue_pos: CachePadded::new(AtomicUsize::new(0)),
        })
    }

    fn push(&self, value: T) -> Result<(), T> {
        let mask = self.mask;
        let mut pos = self.enqueue_pos.load(Relaxed);
        loop {
            let node = &self.buffer[pos & mask];
            let seq = unsafe { (*node.get()).sequence.load(Acquire) };
            let diff: isize = seq as isize - pos as isize;

            if diff == 0 {
                match self
                    .enqueue_pos
                    .compare_exchange_weak(pos, pos + 1, Relaxed, Relaxed)
                {
                    Ok(enqueue_pos) => {
                        debug_assert_eq!(enqueue_pos, pos);
                        unsafe {
                            (*node.get()).value = Some(value);
                            (*node.get()).sequence.store(pos + 1, Release);
                        }
                        break;
                    }
                    Err(enqueue_pos) => pos = enqueue_pos,
                }
            } else if diff < 0 {
                return Err(value);
            } else {
                pos = self.enqueue_pos.load(Relaxed);
            }
        }
        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let mask = self.mask;
        let mut pos = self.dequeue_pos.load(Relaxed);
        loop {
            let node = &self.buffer[pos & mask];
            let seq = unsafe { (*node.get()).sequence.load(Acquire) };
            let diff: isize = seq as isize - (pos + 1) as isize;
            if diff == 0 {
                match self
                    .dequeue_pos
                    .compare_exchange_weak(pos, pos + 1, Relaxed, Relaxed)
                {
                    Ok(dequeue_pos) => {
                        debug_assert_eq!(dequeue_pos, pos);
                        unsafe {
                            let value = (*node.get()).value.take();
                            (*node.get()).sequence.store(pos + mask + 1, Release);
                            return value;
                        }
                    }
                    Err(dequeue_pos) => pos = dequeue_pos,
                }
            } else if diff < 0 {
                return None;
            } else {
                pos = self.dequeue_pos.load(Relaxed);
            }
        }
    }

    fn len(&self) -> usize {
        let dequeue = self.dequeue_pos.load(Relaxed);
        let enqueue = self.enqueue_pos.load(Relaxed);
        if enqueue > dequeue {
            enqueue - dequeue
        } else {
            dequeue - enqueue
        }
    }
}

impl<'a, T: Send> Queue<'a, T> {
    pub fn new(_name: &str) -> Result<Queue<'a, T>, ()> {
        State::with_capacity(QUEUE_SIZE).map(|state| Queue {
            state: Arc::new(state),
            phantom: PhantomData,
        })
    }

    pub fn with_capacity(capacity: usize) -> Result<Queue<'a, T>, ()> {
        Ok(Queue {
            state: Arc::new(State::with_capacity(capacity)?),
            phantom: PhantomData,
        })
    }

    pub fn enqueue(&self, value: T) -> Result<(), T> {
        self.state.push(value)
    }

    pub fn dequeue(&self) -> Option<T> {
        self.state.pop()
    }

    pub fn len(&self) -> usize {
        self.state.len()
    }
}

impl<'a, T: Send> Clone for Queue<'a, T> {
    fn clone(&self) -> Queue<'a, T> {
        Queue {
            state: self.state.clone(),
            phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Queue;
    use alloc::vec;
    use std::sync::mpsc::channel;
    use std::thread;

    #[test]
    fn len() {
        // fill and drain N elements from the queue, with N: 1..=1024
        let q = Queue::<usize>::with_capacity(1024).unwrap();
        assert_eq!(q.len(), 0);
        for i in 1..=1024 {
            for j in 0..i {
                assert_eq!(q.len(), j);
                let _ = q.enqueue(j);
                assert_eq!(q.len(), j + 1);
            }
            for j in (0..i).rev() {
                assert_eq!(q.len(), j + 1);
                let _ = q.dequeue();
                assert_eq!(q.len(), j);
            }
        }

        // steps through each potential wrap-around by filling to N - 1 and
        // draining each time
        let q = Queue::<usize>::with_capacity(1024).unwrap();
        assert_eq!(q.len(), 0);
        for _ in 1..=1024 {
            for j in 0..1023 {
                assert_eq!(q.len(), j);
                let _ = q.enqueue(j);
                assert_eq!(q.len(), j + 1);
            }
            for j in (0..1023).rev() {
                assert_eq!(q.len(), j + 1);
                let _ = q.dequeue();
                assert_eq!(q.len(), j);
            }
        }
    }

    #[test]
    fn test() {
        let nthreads = 8;
        let nmsgs = 1000;
        let q = Queue::with_capacity(nthreads * nmsgs).unwrap();
        assert_eq!(None, q.dequeue());
        let (tx, rx) = channel();

        for _ in 0..nthreads {
            let q = q.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let q = q;
                for i in 0..nmsgs {
                    assert!(q.enqueue(i).is_ok());
                }
                tx.send(()).unwrap();
            });
        }

        let mut completion_rxs = vec![];
        for _ in 0..nthreads {
            let (tx, rx) = channel();
            completion_rxs.push(rx);
            let q = q.clone();
            thread::spawn(move || {
                let q = q;
                let mut i = 0;
                loop {
                    match q.dequeue() {
                        None => {}
                        Some(_) => {
                            i += 1;
                            if i == nmsgs {
                                break;
                            }
                        }
                    }
                }
                tx.send(i).unwrap();
            });
        }

        for rx in completion_rxs.iter_mut() {
            assert_eq!(nmsgs, rx.recv().unwrap());
        }
        for _ in 0..nthreads {
            rx.recv().unwrap();
        }
    }
}
