use eventuals::*;
use tokio::test;

#[test]
async fn joins_values() {
    let (mut a_writer, a) = Eventual::new();
    let (mut b_writer, b) = Eventual::new();

    a_writer.write("a");
    b_writer.write(1);
    let mut ab = join(a, b).subscribe();

    assert_eq!(Ok(("a", 1)), ab.next().await);

    // Since it's a second code path for the "post-completion" updates
    // go ahead and write again.
    a_writer.write("A");
    assert_eq!(Ok(("A", 1)), ab.next().await);

    b_writer.write(2);
    assert_eq!(Ok(("A", 2)), ab.next().await);
}
