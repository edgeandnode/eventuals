use eventuals::*;
use lazy_static::lazy_static;
use std::{
    sync::{
        atomic::{AtomicU32, Ordering::SeqCst},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::select;
use tokio::test;

#[test]
async fn basic() {
    let (mut writer, eventual) = Eventual::<u32>::new();
    writer.write(5);

    // Format the value and save it in an Arc<String> for
    let format_value = |v| async move { Arc::new(format!("{}", v)) };
    let mut mapped = eventual.map(format_value).subscribe();

    assert_eq!(&mapped.next().await.ok().unwrap().as_str(), &"5");

    writer.write(10);
    assert_eq!(&mapped.next().await.ok().unwrap().as_str(), &"10");

    writer.write(10); // Same value, de-duplicated.
    drop(writer);
    assert_eq!(mapped.next().await, Err(Closed))
}

#[test]
async fn with_retry_works_eventually() {
    let (mut writer, nums) = Eventual::new();
    writer.write(1);

    lazy_static! {
        static ref LOCK: Mutex<()> = Mutex::new(());
        static ref TRIES: AtomicU32 = AtomicU32::new(0);
    }

    // In case this test is run more than once or concurrently for some reason, these
    // need to be here to ensure on the test is run consistently.
    let _lock = LOCK.lock().unwrap();
    TRIES.store(0, SeqCst);

    let start = Instant::now();
    let inviolable = map_with_retry(
        nums,
        // Attempt 5 times on the same value before succeeding.
        move |n| async move {
            assert_eq!(n, 1);
            let attempt = TRIES.fetch_add(1, SeqCst);
            if attempt < 4 {
                Err(attempt)
            } else {
                Ok("ok")
            }
        },
        // Sleep 1ms between failures.
        |_| tokio::time::sleep(Duration::from_millis(1)),
    );

    // Assert that this eventually works
    assert_eq!(inviolable.value().await.unwrap(), "ok");
    let end = Instant::now();
    // Verify the sleeps. In practice this ends up much
    // larger than 5ms.
    assert!(end - start >= Duration::from_millis(5));
}

#[test]
async fn with_retry_gets_new_value() {
    let (mut writer, nums) = Eventual::<u32>::new();
    writer.write(1);

    let inviolable = map_with_retry(
        nums,
        move |n| async move {
            match n {
                1 => Err(()),
                _ => Ok(()),
            }
        },
        // Sleep "forever". In practice this could be a short sleep
        // but we want to show that if a new value is available it
        // is used rather than reconstructing the pipeline.
        |_| tokio::time::sleep(Duration::from_secs(1000000000000)),
    );

    // Assert that this eventually works
    select! {
        _ = inviolable.value() => {
            panic!("Nooooooooo!");
        }
        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
    };
    writer.write(2);
    assert_eq!(inviolable.value().await.unwrap(), ());
}
