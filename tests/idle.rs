use eventuals::*;
use std::time::{Duration, Instant};
use tokio::test;
use tokio::time::sleep;

#[test]
async fn never_idle_in_map() {
    let (mut writer, reader) = Eventual::<u8>::new();
    let mapped = reader.subscribe().map(|_v| async move {
        idle().await;
        panic!("Past idle");
    });

    writer.write(1);

    // The panic should not be hit.
    sleep(Duration::from_millis(10)).await;

    // We can still drop the map, which should make the pipeline idle.
    drop(mapped);
    idle().await;
}

#[test]
async fn idle_after_map() {
    let duration = Duration::from_millis(10);

    idle().await;
    let start = Instant::now();
    let mapped = Eventual::from_value(5).map(move |v| async move {
        sleep(duration).await;
        v
    });
    idle().await;
    assert!(Instant::now() - start > duration);
    assert_eq!(mapped.value_immediate(), Some(5));
    drop(mapped);
}
