use lamedh_http::{
    lambda::{lambda, Context, Error},
    IntoResponse, Request,
};

#[lambda(http)]
#[tokio::main]
async fn main(_: Request, _: Context) -> Result<impl IntoResponse, Error> {
    Ok("👋 world")
}
