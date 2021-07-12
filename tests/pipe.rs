use eventuals::*;
use std::time::Duration;
use tokio::test;

#[test]
async fn produces_side_effect() {
    let (mut handle_writer, handle) = Eventual::new();
    let (mut writer, eventual) = Eventual::new();

    let _pipe = eventual.pipe(move |v| {
        handle_writer.write(v);
    });

    writer.write(1);

    assert_eq!(Ok(1), handle.subscribe().next().await);
}

#[test]
async fn stops_after_drop() {
    let (mut writer, eventual) = Eventual::new();

    let pipe = eventual.pipe(move |v| {
        if v == 2 {
            panic!();
        }
    });

    // TODO: This test passing depends on the sleeps. In part this is because
    // the pipe is in a spawned task. If we want to remove the first sleep so
    // that pipe stops _immediately_ we may have to have pipe check a weak
    // reference to the reader each time it acts. Or, use some version of
    // select! that prefers cancellation over writing in spawn. To remove the
    // second sleep we would need to do some fancy things in this test to know
    // that not only has the callback not been called, but also that it won't
    // ever be called in the future (after the test passes).
    writer.write(1);
    drop(pipe);
    tokio::time::sleep(Duration::from_millis(10)).await;
    writer.write(2);
    tokio::time::sleep(Duration::from_millis(10)).await;
}
