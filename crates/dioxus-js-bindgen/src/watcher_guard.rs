use std::fmt;

/// Unified RAII lifecycle guard for active JavaScript/browser watchers.
///
/// Dropping this guard automatically cancels the asynchronous event receiver task
/// and dispatches a browser-side teardown RPC to unregister the underlying observer
/// (`disconnect()`, `removeEventListener()`, etc.).
pub struct WatcherGuard {
    name: &'static str,
    sub_id: u64,
    task: Option<::dioxus::core::Task>,
    cleaned: bool,
}

impl WatcherGuard {
    /// Creates a new `WatcherGuard` with the given diagnostic name, subscription ID, and optional background task.
    pub fn new(name: &'static str, sub_id: u64, task: Option<::dioxus::core::Task>) -> Self {
        Self {
            name,
            sub_id,
            task,
            cleaned: false,
        }
    }

    /// Returns the static diagnostic name of the watcher (e.g. `"watch_resize"`).
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the unique subscription ID assigned to this watcher.
    pub fn subscription_id(&self) -> u64 {
        self.sub_id
    }

    /// Manually consumes and stops the watcher, immediately running teardown.
    pub fn stop(mut self) {
        self.cleanup();
    }

    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;

        if let Some(task) = self.task.take() {
            if ::dioxus::core::Runtime::try_current().is_some() {
                let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                    task.cancel();
                }));
            }
        }

        crate::internal::dispatch_cleanup(self.sub_id);
    }
}

impl Drop for WatcherGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl fmt::Debug for WatcherGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WatcherGuard")
            .field("name", &self.name)
            .field("sub_id", &self.sub_id)
            .finish()
    }
}
