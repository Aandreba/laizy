use std::{time::Duration};
use laizy::{AsyncLazy, DynAsyncLazy};
use tokio::sync::Mutex;

static VEC : DynAsyncLazy<Mutex<Vec<u8>>> = DynAsyncLazy::new_boxed(init_vec);

#[tokio::test]
async fn noblock () {
    let vec = AsyncLazy::new_boxed(init_vec);
}

async fn init_vec() -> Mutex<Vec<u8>> {
    tokio::time::sleep(Duration::from_secs(2)).await;
    Mutex::new(Vec::<u8>::with_capacity(10))
}