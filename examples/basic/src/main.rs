use ciseaux_client::redis;

#[tokio::main]
async fn main() -> Result<(), redis::RedisError> {
    let client = redis::Client::open("redis://127.0.0.1")?;
    let pool = ciseaux_client::CiseauxSingle::new(client, None).await?;
    // Now create a command, and query it
    pool.query_cmd(
        redis::cmd("SET")
            .arg("ciseaux_hello_world")
            .arg("Bonjour le monde"),
    )
    .await?;
    let hello_val = pool
        .query_cmd::<String>(redis::cmd("GET").arg("ciseaux_hello_world"))
        .await?;
    println!("{}", hello_val);
    Ok(())
}
