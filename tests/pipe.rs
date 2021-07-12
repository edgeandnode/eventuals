use eventuals::*;
use std::sync::Arc;
use tokio::{sync::Notify, test};

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
    let notify = Arc::new(Notify::new());
    struct NotifyOnDrop {
        notify: Arc<Notify>,
    }
    impl Drop for NotifyOnDrop {
        fn drop(&mut self) {
            self.notify.notify_one();
        }
    }
    let notify_on_drop = NotifyOnDrop {
        notify: notify.clone(),
    };

    let pipe = eventual.pipe(move |v| {
        if v == 2 {
            panic!();
        }
        // Notifies if it either passed the panic,
        // or will never be called again.
        notify_on_drop.notify.notify_one();
    });

    // This test passing depends on the notifies. In part this is because
    // the pipe is in a spawned task. If we want to remove the first notify so
    // that pipe stops _immediately_ we may have to have pipe check a weak
    // reference to the reader each time it acts. Or, use some version of
    // select! that prefers cancellation over writing in spawn.
    writer.write(1);
    notify.notified().await;
    drop(pipe);
    notify.notified().await;
    // We know this can't panic, because we have been notified that the
    // closure has been dropped and can't be called again. Unfortunately
    // I can't think of a good way to verify it didn't panic. But, surely
    // it doesn't.
    writer.write(2);
}
