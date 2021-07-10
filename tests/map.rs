use eventuals::*;
use std::sync::Arc;
use tokio::test;

#[test]
async fn basic() {
    let (mut writer, eventual) = Eventual::<u32>::new();
    writer.write(5).unwrap();

    // Format the value and save it in an Arc<String> for
    let format_value = |v| async move { Arc::new(format!("{}", v)) };
    let mut mapped = map(&eventual, format_value).subscribe();

    assert_eq!(&mapped.next().await.ok().unwrap().as_str(), &"5");

    writer.write(10).unwrap();
    assert_eq!(&mapped.next().await.ok().unwrap().as_str(), &"10");

    writer.write(10).unwrap(); // Same value, de-duplicated.
    drop(writer);
    assert_eq!(mapped.next().await, Err(Closed))
}
