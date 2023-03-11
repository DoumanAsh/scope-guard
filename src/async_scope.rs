extern crate std;

use std::boxed::Box;
use std::panic;

use core::future::Future;
use core::any::Any;
use core::pin::Pin;
use core::task;

#[must_use]
///Wraps to propagate panic as error.
///
///## Example
///
///```rust
///use scope_guard::CatchUnwindFut;
///
///async fn my_fut() -> Result<(), bool> {
///    Err(true)
///}
///
///async fn example() {
///    match CatchUnwindFut(my_fut()).await {
///        Ok(Ok(())) => panic!("Success!?"),
///        Ok(Err(res)) => assert!(res),
///        Err(panic) => std::panic::resume_unwind(panic),
///    }
///}
///```
pub struct CatchUnwindFut<F: panic::UnwindSafe>(pub F);

impl<F: Future + panic::UnwindSafe> Future for CatchUnwindFut<F> {
    type Output = Result<F::Output, Box<dyn Any + Send + 'static>>;

    #[inline(always)]
    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let fut = unsafe {
            self.map_unchecked_mut(|this| &mut this.0)
        };

        match panic::catch_unwind(panic::AssertUnwindSafe(|| fut.poll(ctx))) {
            Ok(task::Poll::Pending) => task::Poll::Pending,
            Ok(task::Poll::Ready(res)) => task::Poll::Ready(Ok(res)),
            Err(error) => task::Poll::Ready(Err(error)),
        }
    }
}

///Executes future, making sure to perform cleanup regardless of whether `fut` is successful or
///panics.
///
///## Arguments:
///- `dtor` - Generic callback that accepts `args` as its only incoming parameter;
///- `args` - Generic arguments that are passed as it is to the `dtor`;
///- `fut` - Future to execute before calling `dtor`. Regardless of success, `dtor` is always
///executed.
///
///Returns `Output` of `fut` or panics on error in executing `fut`.
///Regardless of `fut` execution status, `dtor` is always called.
///
///## Example
///
///```rust
///use scope_guard::async_scope;
///
///async fn dtor(_args: ()) {
///    println!("dtor!");
///}
///
///async fn example() {
///    let fut = async {
///        panic!("FAIL")
///    };
///
///    async_scope(dtor, (), fut).await;
///}
///```
pub async fn async_scope<
    R,
    F: Future<Output = R> + panic::UnwindSafe,
    DTORARGS,
    DTOR: Future<Output = ()>,
    DTORFN: FnOnce(DTORARGS) -> DTOR,
>(
    dtor: DTORFN,
    args: DTORARGS,
    fut: F,
) -> R {
    let result = CatchUnwindFut(fut).await;
    let dtor = (dtor)(args);
    dtor.await;
    match result {
        Ok(result) => result,
        Err(error) => std::panic::resume_unwind(error),
    }
}
