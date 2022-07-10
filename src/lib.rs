pub use extension::Extension;
pub use globals::*;
pub use sqlite3_ext_macro::*;
pub use types::*;
pub use value::*;

mod extension;
pub mod ffi;
pub mod function;
mod globals;
pub mod stack_ref;
pub mod static_ext;
mod test_helpers;
mod types;
mod value;
pub mod vtab;

#[repr(transparent)]
pub struct Connection {
    db: ffi::sqlite3,
}

impl Connection {
    /// Convert an SQLite handle into a reference to Connection.
    ///
    /// # Safety
    ///
    /// The behavior of this method is undefined if the passed pointer is not valid.
    pub unsafe fn from_ptr<'a>(db: *mut ffi::sqlite3) -> &'a mut Connection {
        &mut *(db as *mut Connection)
    }

    /// Get the underlying SQLite handle.
    pub fn as_ptr(&self) -> *const ffi::sqlite3 {
        &self.db
    }

    /// Get the underlying SQLite handle, mutably.
    pub fn as_mut_ptr(&mut self) -> *mut ffi::sqlite3 {
        &self.db as *const _ as _
    }
}

/// Indicate the risk level for a function or virtual table.
///
/// It is recommended that all functions and virtual table implementations set a risk level,
/// but the default is [RiskLevel::Innocuous] if TRUSTED_SCHEMA=on and [RiskLevel::DirectOnly]
/// otherwise.
///
/// See [this discussion](https://www.sqlite.org/src/doc/latest/doc/trusted-schema.md) for more
/// details about the motivation and implications.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum RiskLevel {
    /// An innocuous function or virtual table is one that can only read content from the
    /// database file in which it resides, and can only alter the database in which it
    /// resides.
    Innocuous,
    /// A direct-only function or virtual table has side-effects that go outside the
    /// database file in which it lives, or return information from outside of the database
    /// file.
    DirectOnly,
}
