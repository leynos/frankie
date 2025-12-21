//! Shared Tokio runtime helper for integration tests.

use std::cell::RefCell;
use std::rc::Rc;

use tokio::runtime::Runtime;

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
