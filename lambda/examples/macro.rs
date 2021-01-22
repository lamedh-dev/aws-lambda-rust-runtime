use lamedh_runtime::{lambda, Context, Error};
use serde_json::Value;

// #[lambda] attribute removes the need for boilerplate code
// required by `lambda::run(func).await?` as demonstrated in other
// examples.

#[lambda]
#[tokio::main]
async fn main(event: Value, _: Context) -> Result<Value, Error> {
    Ok(event)
}
