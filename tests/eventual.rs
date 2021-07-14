use eventuals::*;
use tokio::{join, test};

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

/*
#[test]
async fn chained_eventuals_drop() {
    // TODO: Get a source eventual, map it, and consume the result, then drop
    // and verify the write fails. This probably doesn't work because the map
    // eventual wouldn't notice it was dead because it doesn't write?. It might
    // work, it just would be delayed and drop one "layer" at a time. We could
    // get it to work by notifying the eventual (complicating the API, requiring
    // a join!) or maybe with something fancy using weakrefs for eg: map so that
    // intermediates transiently hold values (seems complicated either way).
    todo!();
}
*/
