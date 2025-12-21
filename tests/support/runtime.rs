//! Shared Tokio runtime helper for integration tests.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use rstest_bdd::Slot;
use tokio::runtime::Runtime;
use wiremock::MockServer;

/// Shared runtime wrapper that can be stored in an `rstest-bdd` Slot.
#[derive(Clone)]
pub struct SharedRuntime(Rc<RefCell<Runtime>>);

impl SharedRuntime {
    pub fn new(runtime: Runtime) -> Self {
        Self(Rc::new(RefCell::new(runtime)))
    }

    pub fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.0.borrow().block_on(future)
    }
}

/// Ensures a Tokio runtime and Wiremock server are initialised.
///
/// # Errors
///
/// Returns an error if the Tokio runtime cannot be created or if the slots behave unexpectedly.
pub fn ensure_runtime_and_server(
    runtime: &Slot<SharedRuntime>,
    server: &Slot<MockServer>,
) -> Result<SharedRuntime, io::Error> {
    if runtime.with_ref(|_| ()).is_none() {
        runtime.set(SharedRuntime::new(Runtime::new()?));
    }

    let shared_runtime = runtime
        .get()
        .ok_or_else(|| io::Error::other("runtime not initialised after set"))?;

    if server.with_ref(|_| ()).is_none() {
        server.set(shared_runtime.block_on(MockServer::start()));
    }

    Ok(shared_runtime)
}
