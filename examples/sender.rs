use shmem_queue::Sender;

#[derive(Debug, Default, Clone, Copy)]
struct Message {
    secret: usize,
    random_number: usize,
}

fn main() {
    let mut message = Message {
        secret: 0,
        random_number: 0,
    };
    let iter = 10 * 1024;
    let sender = Sender::<Message>::new("queue");
    for i in 0..iter {
        message.secret = 0xDEADBEEF;
        message.random_number = i;
        sender.send(message);
    }
}
