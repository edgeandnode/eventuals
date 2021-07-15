use eventuals::*;
use std::{
    error::Error,
    future,
    sync::{Arc, Mutex},
};
use tokio::{sync::Notify, test};

#[test]
async fn basic() {
    let (mut writer, numbers) = Eventual::<u32>::new();
    let notify_read = Arc::new(Notify::new());
    let notify_write1 = notify_read.clone();
    let notify_write2 = notify_read.clone();

    let errors = Arc::new(Mutex::new(vec![]));
    let errors_writer = errors.clone();

    let validated: Eventual<Result<u32, u32>> = numbers.map(|n| match n % 2 {
        0 => future::ready(Ok(n)),
        _ => future::ready(Err(n)),
    });

    let even_numbers = handle_errors(validated, move |err: u32| {
        println!("Err: {}", err);
        let mut errors = errors_writer.lock().unwrap();
        errors.push(err.clone());
        notify_write1.notify_one()
    });
    let _pipe = even_numbers
        .subscribe()
        .pipe(move |_| notify_write2.notify_one());

    for n in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
        writer.write(n);
        // This test requires the notify because the result is not immediately
        // available. We need to wait for handle_errors to complete. In general,
        // Eventuals do not guarantee that the latest value is available
        // immediately but just that it will eventually become the latest value.
        // This is not a problem for real use-cases but is for tests. So, without
        // this notify we can see a previous value.
        notify_read.notified().await;
        assert_eq!(even_numbers.value().await.ok().unwrap(), (n / 2) * 2);
    }

    assert_eq!(*errors.lock().unwrap(), vec![1, 3, 5, 7, 9]);
}

#[test]
async fn complex_err_with_ptr() {
    let (mut string_writer, strings) = Eventual::<&'static str>::new();

    // Exists to wrap the err in some "dynamic" error type that does not
    // impl Value
    fn bad_parse(s: &str) -> Result<u32, Box<dyn Error + Send + Sync>> {
        let result: Result<u32, std::num::ParseIntError> = s.parse();
        result.map_err(|e| e.into())
    }

    // But if we take the sad dynamic error type and use Ptr on it
    // we impl Value.
    let numbers: Eventual<Result<u32, _>> =
        strings.map(|s| async move { bad_parse(s).map_err(Ptr::new) });

    let (mut errors_writer, only_errors) = Eventual::new();
    let only_numbers = numbers
        .subscribe()
        .handle_errors(move |e| errors_writer.write(e));

    string_writer.write("1");

    assert_eq!(only_numbers.value().await, Ok(1));
    string_writer.write("a");
    assert_eq!(
        &format!("{}", only_errors.value().await.unwrap()),
        "invalid digit found in string"
    );
    assert_eq!(only_numbers.value().await, Ok(1));
}
