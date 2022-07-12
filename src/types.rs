use super::{ffi, sqlite3_require_version};
use std::os::raw::{c_char, c_int};

/// Alias for [Error::Sqlite]\([ffi::SQLITE_NOMEM]\).
pub const SQLITE_NOMEM: Error = Error::Sqlite(ffi::SQLITE_NOMEM);
/// Alias for [Error::Sqlite]\([ffi::SQLITE_NOTFOUND]\).
pub const SQLITE_NOTFOUND: Error = Error::Sqlite(ffi::SQLITE_NOTFOUND);
/// Alias for [Error::Sqlite]\([ffi::SQLITE_EMPTY]\).
pub const SQLITE_EMPTY: Error = Error::Sqlite(ffi::SQLITE_EMPTY);
/// Alias for [Error::Sqlite]\([ffi::SQLITE_CONSTRAINT]\).
pub const SQLITE_CONSTRAINT: Error = Error::Sqlite(ffi::SQLITE_CONSTRAINT);
/// Alias for [Error::Sqlite]\([ffi::SQLITE_MISUSE]\).
pub const SQLITE_MISUSE: Error = Error::Sqlite(ffi::SQLITE_MISUSE);
/// Alias for [Error::Sqlite]\([ffi::SQLITE_RANGE]\).
pub const SQLITE_RANGE: Error = Error::Sqlite(ffi::SQLITE_RANGE);

#[derive(Clone, Eq, PartialEq)]
pub enum Error {
    /// An error returned by SQLite.
    Sqlite(i32),
    /// A string received from SQLite contains invalid UTF-8, and cannot be converted to a
    /// `&str`.
    Utf8Error(std::str::Utf8Error),
    /// A string being passed from Rust to SQLite contained an interior nul byte.
    NulError(std::ffi::NulError),
    /// Caused by an attempt to use an API that is not supported in the current version of
    /// SQLite.
    VersionNotSatisfied(std::os::raw::c_int),
    /// An arbitrary string error message. This is never generated by SQLite or
    /// sqlite3_ext, but can be used by consumers of this crate to cause SQLite to fail
    /// with a particular error message.
    Module(String),
    /// The result was not necessary to produce because it is an unchanged column in an
    /// UPDATE operation. See [ValueRef::nochange](crate::ValueRef::nochange) for details.
    NoChange,
}

impl Error {
    /// Convert the return of an SQLite function into a Result\<()\>. This method properly
    /// handles the non-error result codes (SQLITE_OK, SQLITE_ROW, and SQLITE_DONE).
    pub fn from_sqlite(rc: i32) -> Result<()> {
        match rc {
            ffi::SQLITE_OK | ffi::SQLITE_ROW | ffi::SQLITE_DONE => Ok(()),
            _ => Err(Error::Sqlite(rc)),
        }
    }

    pub(crate) fn into_sqlite(self, msg: *mut *mut c_char) -> c_int {
        match self {
            Error::Sqlite(code) => code,
            e @ Error::Utf8Error(_)
            | e @ Error::NulError(_)
            | e @ Error::VersionNotSatisfied(_)
            | e @ Error::Module(_)
            | e @ Error::NoChange => {
                if !msg.is_null() {
                    if let Ok(s) = ffi::str_to_sqlite3(&format!("{}", e)) {
                        unsafe { *msg = s };
                    }
                }
                ffi::SQLITE_ERROR
            }
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sqlite(i) => {
                let errstr: Result<&str> = sqlite3_require_version!(3_007_015, unsafe {
                    std::ffi::CStr::from_ptr(ffi::sqlite3_errstr(*i))
                        .to_str()
                        .map_err(Error::Utf8Error)
                });
                match errstr {
                    Ok(s) => write!(f, "{}", s),
                    _ => write!(f, "SQLite error {}", i),
                }
            }
            Error::Utf8Error(e) => e.fmt(f),
            Error::NulError(e) => e.fmt(f),
            Error::Module(s) => write!(f, "{}", s),
            Error::VersionNotSatisfied(v) => write!(
                f,
                "requires SQLite version {}.{}.{} or above",
                v / 1_000_000,
                (v / 1000) % 1000,
                v % 1000
            ),
            Error::NoChange => write!(f, "invalid Error::NoChange"),
        }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sqlite(i) => {
                let errstr: Result<&str> = sqlite3_require_version!(3_007_015, unsafe {
                    std::ffi::CStr::from_ptr(ffi::sqlite3_errstr(*i))
                        .to_str()
                        .map_err(Error::Utf8Error)
                });
                match errstr {
                    Ok(s) => f.debug_tuple("Sqlite").field(&i).field(&s).finish(),
                    _ => f.debug_tuple("Sqlite").field(&i).finish(),
                }
            }
            Error::Utf8Error(e) => f.debug_tuple("Utf8Error").field(&e).finish(),
            Error::NulError(e) => f.debug_tuple("NulError").field(&e).finish(),
            Error::Module(s) => f.debug_tuple("Module").field(&s).finish(),
            Error::VersionNotSatisfied(v) => {
                f.debug_tuple("VersionNotSatisfied").field(&v).finish()
            }
            Error::NoChange => f.debug_tuple("NoChange").finish(),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::str::Utf8Error> for Error {
    fn from(err: std::str::Utf8Error) -> Self {
        Self::Utf8Error(err)
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(err: std::ffi::NulError) -> Self {
        Self::NulError(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
