use eventuals::*;
use std::time::Duration;
use tokio::test;

#[test]
async fn timer_awaits() {
    let interval = Duration::from_millis(2);
    let timer = timer(interval);
    let mut reader = timer.subscribe();
    let start = reader.next().await.unwrap();
    let end = reader.next().await.unwrap();
    assert!(end - start >= interval);
}
