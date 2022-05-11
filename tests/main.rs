use std::{sync::{Mutex}};
use laizy::{Lazy};

static SYNC : Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(Vec::with_capacity(10)));

#[test]
fn simple () {
    let vec = SYNC.lock().unwrap();
    println!("{}", vec.capacity());
}

#[test]
fn threaded () {
    let mut handles = Vec::new();
    for i in 1..=4 {
       handles.push(std::thread::spawn(move || {
            let mut vec = SYNC.lock().unwrap();
            vec.push(i);
            println!("{vec:?}")
        }));
    }

    let mut vec = SYNC.lock().unwrap();
    vec.push(0);
    println!("{vec:?}");
    drop(vec);

    for handle in handles {
        handle.join().unwrap();
    }
}