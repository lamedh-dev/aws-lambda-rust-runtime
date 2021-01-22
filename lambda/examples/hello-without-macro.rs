use lamedh_runtime::{handler_fn, run, Context, Error};
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler_fn(func)).await?;
    Ok(())
}

async fn func(event: Value, _: Context) -> Result<Value, Error> {
    let first_name = event["firstName"].as_str().unwrap_or("world");

    Ok(json!({ "message": format!("Hello, {}!", first_name) }))
}
