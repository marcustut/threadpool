# threadpool

*Manage a pool of dynamically spawned threads with shared state.* 

This library is a very minimal implementation using 
only `std::thread` and `std::sync::mpsc`. Only dependencies used are `thiserror` and `tracing` which can be avoided if 
wanted to, in future it might be better to gate `tracing` behind a feature flag.

### Installing

At the moment there are no plans of publishing the library to [crates.io](https://crates.io), hence if you want to use it, 
just include it in your `Cargo.toml`:

```toml
threadpool = { git = "https://github.com/marcustut/threadpool.git", branch = "master" }
```

### Usage

Using the threadpool is straightforward, you only need to have two things: 

- `id`
- `state`

Both of these has no concrete types, you can use anything you want as long as the `id` satisfies these traits `std::fmt::Debug + Clone + Eq + std::hash::Hash` 
and similarly there is no restriction to `state` as long as it satisfies the `Clone` trait. 

One downside to the current implementation is that we require `state` to be cloneable and each time when a new worker thread 
is spawn, the pool simply clones the state for the new worker. Hence, if you need mutable state you have to use wrap your 
state in `Arc<Mutex<T>>` or you can use [dashmap](https://docs.rs/dashmap/latest/dashmap/) or other data structures that 
does not need `&mut self` to gain write access. Another option is to use atomic types but these are only for primitive types 
such as `i32`, `u64`, etc.

```rust
use std::{sync::{Arc, Mutex}, collections::HashMap};

struct User {
    name: String
}

type ThreadId = (u64, String);

#[derive(Clone)]
struct ThreadPoolState {
    users: Arc<Mutex<HashMap<u64, User>>>,
}

fn main() {
    let pool = threadpool::ThreadPool::new(ThreadPoolState {
        users: Arc::new(Mutex::new(HashMap::new()))
    });
    
    pool.spawn((0, "worker 1"), say_hello("world"));
    pool.spawn((1, "worker 2"), say_hello("Rust"));
}

fn say_hello(
    name: String,
) -> Arc<
    dyn Fn(ThreadId, ThreadPoolState, std::sync::mpsc::Receiver<()>) -> std::thread::JoinHandle<()>
    + Send
    + Sync,
> {
    Arc::new(move |id, state, shutdown_rx| {
        println!("Hello {}...", name)
    })
}
```

If you want to do asynchronous tasks inside the threadpool, one trick is to create an async runtime before creating the 
threadpool and pass a handle in as a state. For example, you can do this with [tokio](https://tokio.rs/).

```rust
type ThreadId = (u64, String);

#[derive(Clone)]
struct ThreadPoolState {
    users: Arc<Mutex<HashMap<u64, User>>>,
    handle: tokio::runtime::Handle,
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let pool = threadpool::ThreadPool::new(ThreadPoolState {
        users: Arc::new(Mutex::new(HashMap::new())),
        handle: rt.handle().clone(),
    });

    pool.spawn((0, "worker 1"), say_hello_async("world"));
    pool.spawn((1, "worker 2"), say_hello_async("Rust"));
}

fn say_hello_async(
    name: String,
) -> Arc<
    dyn Fn(ThreadId, ThreadPoolState, std::sync::mpsc::Receiver<()>) -> std::thread::JoinHandle<()>
    + Send
    + Sync,
> {
    Arc::new(move |id, state, shutdown_rx| {
        state.rt.block_on(async move {
            println!("Hello {}...", name)
        });
    })
}
```
