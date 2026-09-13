//! Stratum 8: foreign code, and the only `unsafe` in the language.
//!
//! `spec/09-prelude.md` §9.8 and §9.8.1, and `spec/01-strata.md` §1.7. A crate
//! rather than a module, because the workspace sets `unsafe_code = "forbid"`
//! and `forbid` exists so that it cannot be locally undone.
//!
//! `unsafe` lives only where the language ends, and there are two ends. This
//! is where Nether C calls out; `nether-wasm` is where something else calls
//! in. Those two crates, and no others.
//!
//! Nothing here is safe and nothing here pretends to be. §9.8.1 says as much
//! in the specification: a callee that keeps a pointer after it returns has
//! kept a pointer to something it does not own, and no wording stops it. That
//! is what stratum 8 *is*, and it is why §1.7 marks the trace rather than
//! trying to make the call safe.

use std::ffi::{CString, c_char, c_int, c_void};
use std::path::Path;

// `dlopen`, `dlsym`, `dlclose`, `dlerror`. Three declarations and an error
// string, which is less than a binding crate and all of what is needed.
unsafe extern "C" {
    fn dlopen(file: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
    fn dlerror() -> *const c_char;
}

/// `RTLD_NOW | RTLD_LOCAL`. Now, so a missing symbol is an error at load
/// rather than a crash at the call; local, so one object's symbols cannot
/// answer for another's by accident.
const NOW_AND_LOCAL: c_int = 0x2;

/// §9.8.1's signature, as Rust spells it.
type Foreign = unsafe extern "C" fn(*const u8, usize, *mut u8, usize, *mut usize) -> i32;

/// One shared object, kept open for as long as symbols may be called in it.
pub struct Object {
    handle: *mut c_void,
    /// What it was loaded from, for saying which object answered.
    pub path: String,
}

/// What went wrong, in the words `dlerror` used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed(pub String);

impl core::fmt::Display for Failed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for Failed {}

/// Whatever `dlerror` has to say, or a sentence saying it said nothing.
fn last_error() -> String {
    // SAFETY: `dlerror` returns a pointer to a static buffer or null, and the
    // string is valid until the next call on this thread. It is copied here.
    let said = unsafe { dlerror() };
    if said.is_null() {
        return "the loader gave no reason".to_owned();
    }
    // SAFETY: non-null, and `dlerror` returns a NUL-terminated string.
    unsafe { std::ffi::CStr::from_ptr(said) }.to_string_lossy().into_owned()
}

impl Object {
    /// Open one shared object. §8.3.3: the invocation said which.
    ///
    /// # Errors
    ///
    /// [`Failed`] with whatever the loader said, which is the only account of
    /// what happened that exists.
    pub fn open(path: &Path) -> Result<Self, Failed> {
        let shown = path.display().to_string();
        let Ok(name) = CString::new(shown.as_bytes()) else {
            return Err(Failed(format!("{shown} has a NUL in it")));
        };
        // SAFETY: `name` is a valid NUL-terminated string that outlives the
        // call. Everything after this point is the loader's to get right, and
        // §1.7 is the specification admitting that.
        let handle = unsafe { dlopen(name.as_ptr(), NOW_AND_LOCAL) };
        if handle.is_null() { Err(Failed(last_error())) } else { Ok(Self { handle, path: shown }) }
    }

    /// Whether this object has that symbol.
    #[must_use]
    pub fn has(&self, symbol: &str) -> bool {
        self.find(symbol).is_some()
    }

    /// The symbol, if this object has it.
    fn find(&self, symbol: &str) -> Option<Foreign> {
        let name = CString::new(symbol).ok()?;
        // SAFETY: `self.handle` came from `dlopen` and has not been closed;
        // `name` is NUL-terminated and outlives the call.
        let found = unsafe { dlsym(self.handle, name.as_ptr()) };
        if found.is_null() {
            return None;
        }
        // SAFETY: this is the transmute stratum 8 is about. Nothing checks
        // that the symbol has §9.8.1's signature, and nothing can: a shared
        // object carries no types. Calling it is undefined behaviour if it
        // does not, which is why a trace that reached here is marked forever.
        Some(unsafe { core::mem::transmute::<*mut c_void, Foreign>(found) })
    }

    /// Call it, by §9.8.1's protocol.
    ///
    /// Twice at most: `1` means the buffer was too small and `*out_len` is
    /// what is needed, and a callee whose answer grows every time it is asked
    /// is a callee that never finishes.
    ///
    /// `Ok((code, bytes))` is the callee's return value and what it wrote.
    ///
    /// # Errors
    ///
    /// [`Failed`] if this object does not have that symbol, or if it asked
    /// twice for more room.
    pub fn call(&self, symbol: &str, args: &[u8]) -> Result<(i32, Vec<u8>), Failed> {
        let Some(function) = self.find(symbol) else {
            return Err(Failed(format!("{} has no `{symbol}`", self.path)));
        };
        // Enough for most answers without asking, and small enough that being
        // wrong costs one more call rather than a reservation.
        let mut out = vec![0u8; 4096];
        let mut wrote = 0usize;
        // SAFETY: none available. `args` and `out` are live for the call and
        // their lengths are what is passed; everything else is §9.8.1 asking
        // the callee to behave and §1.7 recording that it might not.
        let code = unsafe {
            function(args.as_ptr(), args.len(), out.as_mut_ptr(), out.len(), &raw mut wrote)
        };
        if code != 1 {
            out.truncate(wrote.min(out.len()));
            return Ok((code, out));
        }

        let mut out = vec![0u8; wrote];
        let mut wrote = 0usize;
        // SAFETY: as above, with a buffer of the size the callee asked for.
        let code = unsafe {
            function(args.as_ptr(), args.len(), out.as_mut_ptr(), out.len(), &raw mut wrote)
        };
        if code == 1 {
            return Err(Failed(format!("`{symbol}` asked for more room twice")));
        }
        out.truncate(wrote.min(out.len()));
        Ok((code, out))
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        // SAFETY: `self.handle` came from `dlopen` and is closed once, here.
        unsafe { dlclose(self.handle) };
    }
}
