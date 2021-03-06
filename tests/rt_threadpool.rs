use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use futures_01::future::Future as Future01;
use futures_util::compat::Future01CompatExt;
use tokio_compat::runtime;

#[test]
fn can_run_01_futures() {
    let future_ran = Arc::new(AtomicBool::new(false));
    let ran = future_ran.clone();
    runtime::run(futures_01::future::lazy(move || {
        future_ran.store(true, Ordering::SeqCst);
        Ok(())
    }));
    assert!(ran.load(Ordering::SeqCst));
}

#[test]
fn can_spawn_01_futures() {
    let future_ran = Arc::new(AtomicBool::new(false));
    let ran = future_ran.clone();
    runtime::run(futures_01::future::lazy(move || {
        tokio_01::spawn(futures_01::future::lazy(move || {
            future_ran.store(true, Ordering::SeqCst);
            Ok(())
        }));
        Ok(())
    }));
    assert!(ran.load(Ordering::SeqCst));
}

#[test]
fn can_spawn_std_futures() {
    let future_ran = Arc::new(AtomicBool::new(false));
    let ran = future_ran.clone();
    runtime::run(futures_01::future::lazy(move || {
        tokio_02::spawn(async move {
            future_ran.store(true, Ordering::SeqCst);
        });
        Ok(())
    }));
    assert!(ran.load(Ordering::SeqCst));
}

#[test]
fn tokio_01_timers_work() {
    let future1_ran = Arc::new(AtomicBool::new(false));
    let ran = future1_ran.clone();
    let future1 = futures_01::future::lazy(|| {
        let when = Instant::now() + Duration::from_millis(15);
        tokio_01::timer::Delay::new(when).map(move |_| when)
    })
    .map(move |when| {
        ran.store(true, Ordering::SeqCst);
        assert!(Instant::now() >= when);
    })
    .map_err(|_| panic!("timer should work"));

    let future2_ran = Arc::new(AtomicBool::new(false));
    let ran = future2_ran.clone();
    let future2 = async move {
        let when = Instant::now() + Duration::from_millis(10);
        tokio_01::timer::Delay::new(when).compat().await.unwrap();
        ran.store(true, Ordering::SeqCst);
        assert!(Instant::now() >= when);
    };

    runtime::run(futures_01::future::lazy(move || {
        tokio_02::spawn(future2);
        tokio_01::spawn(future1);
        Ok(())
    }));
    assert!(future1_ran.load(Ordering::SeqCst));
    assert!(future2_ran.load(Ordering::SeqCst));
}

#[test]
fn block_on_01_timer() {
    let mut rt = runtime::Runtime::new().unwrap();
    let when = Instant::now() + Duration::from_millis(10);
    rt.block_on(tokio_01::timer::Delay::new(when)).unwrap();
    assert!(Instant::now() >= when);
}

#[test]
fn block_on_std_01_timer() {
    let mut rt = runtime::Runtime::new().unwrap();
    let when = Instant::now() + Duration::from_millis(10);
    rt.block_on_std(async move {
        tokio_01::timer::Delay::new(when).compat().await.unwrap();
    });
    assert!(Instant::now() >= when);
}

#[test]
fn block_on_01_spawn() {
    let mut rt = runtime::Runtime::new().unwrap();
    // other tests assert that spawned 0.1 tasks actually *run*, all we care
    // is that we're able to spawn it successfully.
    rt.block_on(futures_01::future::lazy(|| {
        tokio_01::spawn(futures_01::future::lazy(|| Ok(())))
    }))
    .unwrap();
}

#[test]
fn block_on_std_01_spawn() {
    let mut rt = runtime::Runtime::new().unwrap();
    // other tests assert that spawned 0.1 tasks actually *run*, all we care
    // is that we're able to spawn it successfully.
    rt.block_on_std(async { tokio_01::spawn(futures_01::future::lazy(|| Ok(()))) });
}

#[test]
fn tokio_02_spawn_blocking_works() {
    let ran = Arc::new(AtomicBool::new(false));
    let ran2 = ran.clone();
    runtime::run_std(async move {
        println!("in future, before blocking");
        tokio_02::task::spawn_blocking(move || {
            println!("in blocking");
            ran.store(true, Ordering::SeqCst);
        })
        .await
        .expect("blocking task panicked!");
        println!("blocking done");
    });
    assert!(ran2.load(Ordering::SeqCst));
}

#[test]
fn tokio_02_block_in_place_works() {
    let ran = Arc::new(AtomicBool::new(false));
    let ran2 = ran.clone();
    runtime::run_std(async move {
        println!("in future, before blocking");
        tokio_02::task::spawn(async move {
            tokio_02::task::block_in_place(move || {
                println!("in blocking");
                ran.store(true, Ordering::SeqCst);
            })
        })
        .await
        .expect("blocking task panicked!");
        println!("blocking done");
    });
    assert!(ran2.load(Ordering::SeqCst));
}

#[test]
fn block_on_twice() {
    // Repro for tokio-rs/tokio-compat#10.
    let mut rt = runtime::Runtime::new().unwrap();
    rt.block_on_std(async {
        tokio_02::spawn(async {}).await.unwrap();
        println!("spawn 1 done")
    });
    println!("block_on 1 done");
    rt.block_on_std(async {
        tokio_02::spawn(async {}).await.unwrap();
        println!("spawn 2 done");
    });
    println!("done");
}

#[test]
fn idle_after_block_on() {
    let mut rt = runtime::Runtime::new().unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    rt.block_on_std(async {
        tokio_02::spawn(async {}).await;
    });
    let ran2 = ran.clone();
    rt.spawn_std(async move {
        tokio_02::task::yield_now().await;
        ran2.store(true, Ordering::SeqCst);
    });
    rt.shutdown_on_idle();
    assert!(ran.load(Ordering::SeqCst));
}

#[test]
fn enter_exposed() {
    let rt = runtime::Runtime::new().unwrap();
    rt.enter(|| {
        let _handle = tokio_02::runtime::Handle::current();
    });
}

#[test]
fn enter_can_spawn_01_futures() {
    let future_ran = Arc::new(AtomicBool::new(false));
    let ran = future_ran.clone();
    let mut rt = runtime::Runtime::new().unwrap();
    rt.enter(|| {
        tokio_01::spawn(futures_01::future::lazy(move || {
            future_ran.store(true, Ordering::SeqCst);
            Ok(())
        }))
    });

    rt.shutdown_on_idle();
    assert!(ran.load(Ordering::SeqCst));
}
