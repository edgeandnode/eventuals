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
    // TODO: A dbg! was used to verify that the subscriber count goes to 0 and
    // there is no leak. A unit test may be required to make that test permanent.
    let (mut writer, eventual) = Eventual::new();
    let mut read_0 = eventual.subscribe();
    let mut read_1 = eventual.subscribe();
    writer.write(5);
    writer.write(10);
    assert_eq!(read_0.next().await, Ok(10));
    drop(read_0);
    writer.write(1);
    assert_eq!(read_1.next().await, Ok(1));
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
