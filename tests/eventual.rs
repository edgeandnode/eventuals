use eventuals::*;
use std::{sync::Arc, time::Duration};
use tokio::{join, test, time::sleep};

#[test]
async fn dropped_writer_closes() {
    let (writer, eventual) = Eventual::<u32>::new();
    let mut read_0 = eventual.subscribe();
    drop(writer);
    assert_eq!(read_0.next().await, Err(Closed));
}

#[test]
async fn can_observe_value_written_after_subscribe() {
    let (mut writer, eventual) = Eventual::new();
    let mut read_0 = eventual.subscribe();
    writer.write(5);
    assert_eq!(read_0.next().await, Ok(5));
}

#[test]
async fn can_observe_value_written_before_subscribe() {
    let (mut writer, eventual) = Eventual::new();
    writer.write(5);
    let mut read_0 = eventual.subscribe();
    assert_eq!(read_0.next().await, Ok(5));
}

#[test]
async fn only_most_recent_value_is_observed() {
    let (mut writer, eventual) = Eventual::new();
    let mut read_0 = eventual.subscribe();
    writer.write(5);
    writer.write(10);
    assert_eq!(read_0.next().await, Ok(10));
}

#[test]
async fn drop_doesnt_interfere() {
    let (mut writer, eventual) = Eventual::<u32>::new();
    assert_eq!(eventual.subscriber_count(), 0);
    let mut read_0 = eventual.subscribe();
    assert_eq!(eventual.subscriber_count(), 1);
    let mut read_1 = eventual.subscribe();
    assert_eq!(eventual.subscriber_count(), 2);
    writer.write(5);
    writer.write(10);
    assert_eq!(read_0.next().await, Ok(10));
    drop(read_0);
    assert_eq!(eventual.subscriber_count(), 1);
    writer.write(1);
    // The main point of the test is this line - after
    // dropping one subscriber we still have our subscriber.
    assert_eq!(read_1.next().await, Ok(1));
    drop(read_1);
    // It is also useful to verify that the above test passed
    // even though drop is in fact working.
    assert_eq!(eventual.subscriber_count(), 0);
}

#[test]
async fn can_message_pass() {
    let (mut writer_a, eventual_a) = Eventual::<u32>::new();
    let (mut writer_b, eventual_b) = Eventual::<u32>::new();

    let mut eventual_a = eventual_a.subscribe();
    let mut eventual_b = eventual_b.subscribe();

    let b = tokio::spawn(async move {
        let mut sum = 0;
        while let Ok(v) = eventual_a.next().await {
            sum += v;
            writer_b.write(v + 1);
        }
        sum
    });

    let a = tokio::spawn(async move {
        writer_a.write(0);
        let first = eventual_b.next().await.unwrap();
        writer_a.write(first + 1);
        let second = eventual_b.next().await.unwrap();
        writer_a.write(second + 1);
        assert_eq!(eventual_b.next().await, Ok(5));
        drop(writer_a);
        assert_eq!(eventual_b.next().await, Err(Closed));
    });

    let (a, b) = join!(a, b);
    assert!(a.is_ok());
    assert_eq!(b.unwrap(), 6);
}

// Ensures that eventuals will drop all the way down the chain "immediately"
#[test]
async fn chained_eventuals_drop() {
    let (mut writer, source) = Eventual::new();
    let source = Arc::new(source);
    let mut new_source = source.clone();
    let mut i = 0;
    let mapped = loop {
        new_source = Arc::new(new_source.subscribe().map(|v: u32| async move { v + 1 }));
        i += 1;
        if i == 25 {
            break new_source;
        }
    };

    assert_eq!(source.subscriber_count(), 1);
    assert_eq!(mapped.subscriber_count(), 0);

    writer.write(5);
    assert_eq!(mapped.value().await, Ok(30));

    assert_eq!(source.subscriber_count(), 1);
    drop(mapped);
    // Dropping doesn't happen on the same thread, but
    // it still should happen before we write a value.
    sleep(Duration::from_millis(1)).await;
    assert_eq!(source.subscriber_count(), 0);
}
