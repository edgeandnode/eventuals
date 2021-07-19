#![allow(deprecated)]
use eventuals::*;
use tokio::test;

#[test]
async fn it_works() {
    let (mut a_writer, a) = Eventual::<&'static str>::new();
    let (mut b_writer, b) = Eventual::<&'static str>::new();

    let either = select((a, b));

    let mut values = either.subscribe();
    a_writer.write("a");
    assert_eq!(values.next().await, Ok("a"));
    b_writer.write("b");
    assert_eq!(values.next().await, Ok("b"));

    drop(b_writer);
    drop(a_writer);
    assert_eq!(values.next().await, Err(Closed));
}
