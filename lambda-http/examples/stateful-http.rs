use lamedh_http::{
    handler, lambda,
    lambda::{Context, Error},
    Handler, Request,
};
use std::{
    boxed::Box,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

struct AppState {
    pub counter: AtomicUsize,
}

struct StatefulHttpHandler {
    pub state: Arc<AppState>,
}

impl Handler for StatefulHttpHandler {
    type Response = String;
    type Error = Error;
    type Fut = Pin<Box<dyn Future<Output = Result<Self::Response, Error>> + Send + 'static>>;

    fn call(&mut self, request: Request, context: Context) -> Self::Fut {
        let state = Arc::clone(&self.state);
        let fut = handle_request(request, context, state);
        Box::pin(fut)
    }
}

async fn handle_request(_request: Request, _: Context, state: Arc<AppState>) -> Result<String, Error> {
    // increment the counter (from https://doc.rust-lang.org/std/sync/struct.Arc.html)
    let val = state.counter.fetch_add(1, Ordering::SeqCst);
    Ok(format!("{}", &val))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // create state struct that will persist between requests
    let app_state = AppState {
        counter: AtomicUsize::new(0),
    };

    // wrap in an Arc for thread safety and assign to the StatefulHttpHandler
    let stateful_handler = StatefulHttpHandler {
        state: Arc::new(app_state),
    };

    // run the handler in the lambda runtime
    lambda::run(handler(stateful_handler)).await
}
