//! Extension pointer helpers.
//!
//! CLAP extensions are exposed as `*const clap_plugin_XYZ` — null when the
//! plugin doesn't implement them. Each method on the plugin also stores fn
//! pointers as `Option<unsafe extern "C" fn(...) -> _>`. The host code
//! therefore repeats this shape dozens of times:
//!
//! ```ignore
//! if self.extensions.X.Y.is_null() { return None; }
//! let ext = unsafe { &*self.extensions.X.Y };
//! let f = ext.some_fn?;
//! ```
//!
//! `ExtPtr` gives those two operations as a single `.get()` chain.

/// Dereference a cached extension pointer to a shared reference, or `None`
/// if the plugin does not implement the extension.
///
/// # Safety
/// The caller must ensure the extension pointer outlives the returned
/// reference. In practice this is true for any borrow of `&self` on
/// `ClapInstance`, since the extension pointers are owned by the plugin
/// and the plugin outlives `self`.
pub(crate) unsafe fn opt<'a, T>(ptr: *const T) -> Option<&'a T> {
    if ptr.is_null() {
        None
    } else {
        Some(&*ptr)
    }
}
