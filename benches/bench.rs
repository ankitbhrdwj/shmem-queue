use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

const ITERATIONS: usize = 50_000_000;

#[derive(Debug, Default, Clone, Copy)]
struct Message {
    _secret: usize,
    _random_number: usize,
    _padding: usize,
}

#[cfg(feature = "mpsc")]
fn unbatched() {
    let active_sender = Arc::new(AtomicBool::new(true));

    let rx_flag = active_sender.clone();
    let rx_thread = std::thread::spawn(move || {
        let reciver = shmem_queue::Receiver::<Message>::new("queue");
        while rx_flag.load(std::sync::atomic::Ordering::Acquire) {
            let _ = reciver.try_recv();
        }
    });

    let tx_flag = active_sender.clone();
    let tx_thread = std::thread::spawn(move || {
        let sender = shmem_queue::Sender::<Message>::new("queue");

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            sender.send(Message::default());
        }
        let elapsed_time = start.elapsed().as_nanos() as u64;
        println!(
            "{} ns/iter, {} iters/s",
            elapsed_time / ITERATIONS as u64,
            (ITERATIONS as f64 * 1e9) / elapsed_time as f64
        );

        tx_flag.store(false, std::sync::atomic::Ordering::Relaxed);
    });
    _ = rx_thread.join();
    _ = tx_thread.join();
}

#[cfg(feature = "spsc")]
fn batched(batch_size: usize) {
    let active_sender = Arc::new(AtomicBool::new(true));

    let rx_flag = active_sender.clone();
    let rx_thread = std::thread::spawn(move || {
        let reciver = shmem_queue::Receiver::<Message>::new("queue");
        while rx_flag.load(std::sync::atomic::Ordering::Acquire) {
            let _ = reciver.try_recv_batch();
        }
    });

    let tx_flag = active_sender.clone();
    let tx_thread = std::thread::spawn(move || {
        let sender = shmem_queue::Sender::<Message>::new("queue");

        let start = Instant::now();
        for _ in 0..ITERATIONS / batch_size {
            let mut batch = Vec::with_capacity(batch_size);
            for _ in 0..batch_size {
                batch.push(Message::default());
            }
            sender.send_batch(batch);
        }
        let elapsed_time = start.elapsed().as_nanos() as u64;
        println!(
            "{} BS, {} ns/iter, {} iters/s",
            batch_size,
            elapsed_time / ITERATIONS as u64,
            (ITERATIONS as f64 * 1e9) / elapsed_time as f64
        );

        tx_flag.store(false, std::sync::atomic::Ordering::Relaxed);
    });
    _ = rx_thread.join();
    _ = tx_thread.join();
}

fn main() {
    #[cfg(feature = "mpsc")]
    {
        unbatched();
    }

    #[cfg(feature = "spsc")]
    {
        for i in 0..=5 {
            let batch_size = 1 << i;
            batched(batch_size);
        }
    }
}
