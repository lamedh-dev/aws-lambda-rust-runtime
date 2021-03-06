#![deny(missing_docs)]

//! Enriches the `lambda` crate with [`http`](https://github.com/hyperium/http)
//! types targeting AWS [ALB](https://docs.aws.amazon.com/elasticloadbalancing/latest/application/introduction.html), [API Gateway](https://docs.aws.amazon.com/apigateway/latest/developerguide/welcome.html) REST and HTTP API lambda integrations.
//!
//! This crate abstracts over all of these trigger events using standard [`http`](https://github.com/hyperium/http) types minimizing the mental overhead
//! of understanding the nuances and variation between trigger details allowing you to focus more on your application while also giving you to the maximum flexibility to
//! transparently use whichever lambda trigger suits your application and cost optimiztions best.
//!
//! # Examples
//!
//! ## Hello World
//!
//! `lambda_http` handlers adapt to the standard `lambda::Handler` interface using the [`handler`](fn.handler.html) function.
//!
//! The simplest case of an http handler is a function of an `http::Request` to a type that can be lifted into an `http::Response`.
//! You can learn more about these types [here](trait.IntoResponse.html).
//!
//! Adding an `#[lambda(http)]` attribute to a `#[tokio::run]`-decorated `main` function will setup and run the Lambda function.
//!
//! Note: this comes at the expense of any onetime initialization your lambda task might find value in.
//! The full body of your `main` function will be executed on **every** invocation of your lambda task.
//!
//! ```rust,no_run
//! use lamedh_http::{
//!    lambda::{lambda, Context, Error},
//!    IntoResponse, Request,
//! };
//!
//! #[lambda(http)]
//! #[tokio::main]
//! async fn main(_: Request, _: Context) -> Result<impl IntoResponse, Error> {
//!     Ok("👋 world!")
//! }
//! ```
//!
//! ## Hello World, Without Macros
//!
//! For cases where your lambda might benfit from one time function initializiation might
//! prefer a plain `main` function and invoke `lamedh_runtime::run` explicitly in combination with the [`handler`](fn.handler.html) function.
//! Depending on the runtime cost of your dependency bootstrapping, this can reduce the overall latency of your functions execution path.
//!
//! ```rust,no_run
//! use lamedh_http::{handler, lambda::{self, Error}};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // initialize dependencies once here for the lifetime of your
//!     // lambda task
//!     lamedh_runtime::run(handler(|request, context| async { Ok("👋 world!") })).await?;
//!     Ok(())
//! }
//!
//! ```
//!
//! ## Leveraging trigger provided data
//!
//! You can also access information provided directly from the underlying trigger events, like query string parameters,
//! with the [`RequestExt`](trait.RequestExt.html) trait.
//!
//! ```rust,no_run
//! use lamedh_http::{handler, lambda::{self, Context, Error}, IntoResponse, Request, RequestExt};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     lamedh_runtime::run(handler(hello)).await?;
//!     Ok(())
//! }
//!
//! async fn hello(
//!     request: Request,
//!     _: Context
//! ) -> Result<impl IntoResponse, Error> {
//!     Ok(format!(
//!         "hello {}",
//!         request
//!             .query_string_parameters()
//!             .get("name")
//!             .unwrap_or_else(|| "stranger")
//!     ))
//! }
//! ```

// only externed because maplit doesn't seem to play well with 2018 edition imports
#[cfg(test)]
#[macro_use]
extern crate maplit;

pub use http::{self, Response};
pub use lamedh_attributes::lambda;
pub use lamedh_runtime::{self as lambda, Context, Error, Handler as LambdaHandler};

use aws_lambda_events::encodings::Body;
use aws_lambda_events::event::apigw::ApiGatewayProxyRequest;

pub mod ext;
pub mod request;
mod response;
mod strmap;
pub use crate::{ext::RequestExt, response::IntoResponse, strmap::StrMap};
use crate::{
    request::{self as lambda_request, LambdaRequest, RequestOrigin},
    response::LambdaResponse,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

/// Type alias for `http::Request`s with a fixed [`Body`](enum.Body.html) type
pub type Request = http::Request<Body>;

/// Functions serving as ALB and API Gateway REST and HTTP API handlers must conform to this type.
///
/// This can be viewed as a `lambda::Handler` constrained to `http` crate `Request` and `Response` types
pub trait Handler: Sized {
    /// The type of Error that this Handler will return
    type Error;
    /// The type of Response this Handler will return
    type Response: IntoResponse;
    /// The type of Future this Handler will return
    type Fut: Future<Output = Result<Self::Response, Self::Error>> + 'static;
    /// Function used to execute handler behavior
    fn call(&mut self, event: Request, context: Context) -> Self::Fut;
}

/// An implementation of `Handler` for a given closure return a `Future` representing the computed response
impl<F, R, Fut> Handler for F
where
    F: Fn(Request, Context) -> Fut,
    R: IntoResponse,
    Fut: Future<Output = Result<R, Error>> + Send + 'static,
{
    type Response = R;
    type Error = Error;
    type Fut = Fut;
    fn call(&mut self, event: Request, context: Context) -> Self::Fut {
        (self)(event, context)
    }
}

#[doc(hidden)]
pub struct TransformResponse<R, E> {
    request_origin: RequestOrigin,
    fut: Pin<Box<dyn Future<Output = Result<R, E>>>>,
}

impl<R, E> Future for TransformResponse<R, E>
where
    R: IntoResponse,
{
    type Output = Result<LambdaResponse, E>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext) -> Poll<Self::Output> {
        match self.fut.as_mut().poll(cx) {
            Poll::Ready(result) => Poll::Ready(
                result.map(|resp| LambdaResponse::from_response(&self.request_origin, resp.into_response())),
            ),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Adapts a [`Handler`](trait.Handler.html) to the `lamedh_runtime::run` interface
///
/// This is an abstract interface that tries to deserialize the request payload
/// in any possible [`request::LambdaRequest`] value.
pub fn handler<H: Handler>(handler: H) -> Adapter<H> {
    Adapter { handler }
}

/// Exists only to satisfy the trait cover rule for `lambda::Handler` impl
///
/// User code should never need to interact with this type directly. Since `Adapter` implements `Handler`
/// It serves as a opaque trait covering type.
///
/// See [this article](http://smallcultfollowing.com/babysteps/blog/2015/01/14/little-orphan-impls/)
/// for a larger explaination of why this is nessessary
pub struct Adapter<H: Handler> {
    handler: H,
}

impl<H: Handler> Handler for Adapter<H> {
    type Response = H::Response;
    type Error = H::Error;
    type Fut = H::Fut;
    fn call(&mut self, event: Request, context: Context) -> Self::Fut {
        self.handler.call(event, context)
    }
}

impl<H: Handler> LambdaHandler<LambdaRequest, LambdaResponse> for Adapter<H> {
    type Error = H::Error;
    type Fut = TransformResponse<H::Response, Self::Error>;
    fn call(&mut self, event: LambdaRequest, context: Context) -> Self::Fut {
        let request_origin = event.request_origin();
        let fut = Box::pin(self.handler.call(event.into(), context));
        TransformResponse { request_origin, fut }
    }
}

/// Adapts a [`Handler`](trait.Handler.html) to the `lamedh_runtime::run` interface
///
/// This is a concrete interface that tries to deserialize the request payload
/// into an AWS API Gateway Proxy definition. This definition is the same that the
/// AWS Invoke API uses to send invocation requests to lambda.
pub fn proxy_handler<H: Handler>(handler: H) -> ProxyAdapter<H> {
    ProxyAdapter { handler }
}

/// Exists only to satisfy the trait cover rule for `lambda::Handler` impl
///
/// User code should never need to interact with this type directly. Since `ProxyAdapter` implements `Handler`
/// It serves as a opaque trait covering type.
///
/// See [this article](http://smallcultfollowing.com/babysteps/blog/2015/01/14/little-orphan-impls/)
/// for a larger explaination of why this is nessessary
pub struct ProxyAdapter<H: Handler> {
    handler: H,
}

impl<H: Handler> Handler for ProxyAdapter<H> {
    type Response = H::Response;
    type Error = H::Error;
    type Fut = H::Fut;
    fn call(&mut self, event: Request, context: Context) -> Self::Fut {
        self.handler.call(event, context)
    }
}

impl<H: Handler> LambdaHandler<ApiGatewayProxyRequest, LambdaResponse> for ProxyAdapter<H> {
    type Error = H::Error;
    type Fut = TransformResponse<H::Response, Self::Error>;
    fn call(&mut self, event: ApiGatewayProxyRequest, context: Context) -> Self::Fut {
        let request_origin = RequestOrigin::ApiGatewayV1;
        let req = lambda_request::into_proxy_request(event);
        let fut = Box::pin(self.handler.call(req, context));
        TransformResponse { request_origin, fut }
    }
}
