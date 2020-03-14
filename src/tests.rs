// To run tests, you need a running redis instance with no password on 127.0.0.1:6379
#[tokio::test]
async fn try_single() {
    const HELLO_VALUE: &'static str = "qwertyuiop";
    use crate::{single::SingleInit, ConnectionsCount, ReconnectBehavior};
    let redis_client =
        redis::Client::open("redis://127.0.0.1:6379").expect("Failed to create redis::Client");
    let db_pool = {
        let init = SingleInit {
            client: redis_client,
            conns_count: ConnectionsCount::Global(1),
            reconnect_behavior: ReconnectBehavior::NoReconnect,
        };
        init.build().await.expect("Failed to build CiseauxSingle")
    };
    assert!(db_pool
        .query::<_, ()>(redis::Cmd::set("ciseaux_client_tests_hello", HELLO_VALUE))
        .await
        .is_ok());
    let hello = db_pool
        .query::<_, String>(&redis::Cmd::get("ciseaux_client_tests_hello"))
        .await
        .expect("try_single GET failed");
    assert!(hello == HELLO_VALUE);
}
