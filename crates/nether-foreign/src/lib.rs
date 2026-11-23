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

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
#[cfg(not(target_os = "linux"))]
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

#[repr(C)]
struct DlInfo {
    fname: *const c_char,
    fbase: *mut c_void,
    sname: *const c_char,
    saddr: *mut c_void,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct LinkMap {
    addr: usize,
    name: *const c_char,
    ld: *mut c_void,
    next: *mut LinkMap,
    prev: *mut LinkMap,
}

#[cfg(target_os = "linux")]
const RTLD_DI_LINKMAP: c_int = 2;

// `dlopen`, `dlsym`, `dlclose`, `dlerror`, `dladdr` (and Linux `dlinfo`).
// Minimal POSIX dynamic-loader declarations without an external binding crate.
unsafe extern "C" {
    fn dlopen(file: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
    fn dlerror() -> *const c_char;
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
    #[cfg(target_os = "linux")]
    fn dlinfo(handle: *mut c_void, request: c_int, info: *mut c_void) -> c_int;
}

/// `RTLD_NOW | RTLD_LOCAL`. Now, so a missing symbol is an error at load
/// rather than a crash at the call; local, so one object's symbols cannot
/// answer for another's by accident. On macOS `RTLD_LOCAL` is `0x4` and
/// omitting it defaults to `RTLD_GLOBAL`.
#[cfg(target_os = "macos")]
const NOW_AND_LOCAL: c_int = 0x2 | 0x4;
#[cfg(not(target_os = "macos"))]
const NOW_AND_LOCAL: c_int = 0x2;

/// §9.8.1's signature, as Rust spells it.
type Foreign = unsafe extern "C" fn(*const u8, usize, *mut u8, usize, *mut usize) -> i32;

/// One shared object, kept open for as long as symbols may be called in it.
///
/// Calls into it take turns. §9.8.1 promises a callee nothing about threads,
/// so two at once would be a race in code nobody here can see.
pub struct Object {
    handle: Mutex<Handle>,
    #[cfg(not(target_os = "linux"))]
    canonical: Option<PathBuf>,
    /// What it was loaded from, for saying which object answered.
    pub path: String,
}

/// What `dlopen` returned.
struct Handle(*mut c_void);

// SAFETY: a `dlopen` handle names the object for the whole process, and
// `dlsym` and `dlclose` may be called on it from any thread. Calling into the
// object from two threads at once is what is not safe, and the `Mutex` in
// `Object` is held for every use of the handle, so that cannot happen.
unsafe impl Send for Handle {}

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
    unsafe { CStr::from_ptr(said) }.to_string_lossy().into_owned()
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
        let Ok(name) = CString::new(path.as_os_str().as_bytes()) else {
            return Err(Failed(format!("{shown} has a NUL in it")));
        };
        // SAFETY: `name` is a valid NUL-terminated string that outlives the
        // call. Everything after this point is the loader's to get right, and
        // §1.7 is the specification admitting that.
        let handle = unsafe { dlopen(name.as_ptr(), NOW_AND_LOCAL) };
        if handle.is_null() {
            Err(Failed(last_error()))
        } else {
            Ok(Self {
                handle: Mutex::new(Handle(handle)),
                #[cfg(not(target_os = "linux"))]
                canonical: path.canonicalize().ok(),
                path: shown,
            })
        }
    }

    /// Whether this object has that symbol.
    #[must_use]
    pub fn has(&self, symbol: &str) -> bool {
        let handle = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        self.find(&handle, symbol).is_some()
    }

    /// Verify that `found` is defined in the opened shared object itself, not
    /// resolved transitively through `DT_NEEDED` dependencies such as `libc`.
    #[cfg(target_os = "linux")]
    fn defined_in_object(&self, handle: &Handle, found: *mut c_void) -> bool {
        let _ = self;
        let mut lm: *mut LinkMap = core::ptr::null_mut();
        // SAFETY: `handle.0` is an open `dlopen` handle and `RTLD_DI_LINKMAP`
        // writes a `*mut LinkMap` into `lm`.
        if unsafe { dlinfo(handle.0, RTLD_DI_LINKMAP, (&raw mut lm).cast()) } != 0 || lm.is_null() {
            return false;
        }
        let mut info = DlInfo {
            fname: core::ptr::null(),
            fbase: core::ptr::null_mut(),
            sname: core::ptr::null(),
            saddr: core::ptr::null_mut(),
        };
        // SAFETY: `found` is a valid code address returned by `dlsym`.
        if unsafe { dladdr(found.cast_const(), &raw mut info) } == 0 {
            return false;
        }
        // SAFETY: `lm` is non-null and valid while `handle.0` remains open.
        let base = unsafe { (*lm).addr };
        info.fbase as usize == base
    }

    /// Verify that `found` is defined in the opened shared object itself, not
    /// resolved transitively through linked system libraries.
    #[cfg(not(target_os = "linux"))]
    fn defined_in_object(&self, _handle: &Handle, found: *mut c_void) -> bool {
        let Some(expected) = &self.canonical else {
            return false;
        };
        let mut info = DlInfo {
            fname: core::ptr::null(),
            fbase: core::ptr::null_mut(),
            sname: core::ptr::null(),
            saddr: core::ptr::null_mut(),
        };
        // SAFETY: `found` is a valid code address returned by `dlsym`.
        if unsafe { dladdr(found.cast_const(), &raw mut info) } == 0 || info.fname.is_null() {
            return false;
        }
        // SAFETY: `fname` is a non-null NUL-terminated path from `dladdr`.
        let fname = unsafe { CStr::from_ptr(info.fname) };
        let resolved = Path::new(std::ffi::OsStr::from_bytes(fname.to_bytes()));
        resolved.canonicalize().ok().as_ref() == Some(expected)
    }

    /// The symbol, if this object itself defines it.
    fn find(&self, handle: &Handle, symbol: &str) -> Option<Foreign> {
        let name = CString::new(symbol).ok()?;
        // SAFETY: `handle` came from `dlopen` and has not been closed; `name`
        // is NUL-terminated and outlives the call.
        let found = unsafe { dlsym(handle.0, name.as_ptr()) };
        if found.is_null() || !self.defined_in_object(handle, found) {
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
        const MAX_OUTPUT: usize = 64 * 1024 * 1024;
        // Held to the end of the call, both tries: that is the turn.
        let handle = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(function) = self.find(&handle, symbol) else {
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
            if code == 0 && wrote > out.len() {
                return Err(Failed(format!(
                    "`{symbol}` claimed to write {wrote} bytes into a {}-byte buffer",
                    out.len()
                )));
            }
            out.truncate(wrote.min(out.len()));
            return Ok((code, out));
        }

        if wrote > MAX_OUTPUT {
            return Err(Failed(format!(
                "`{symbol}` asked for {wrote} bytes, past the 64 MiB bound"
            )));
        }
        let mut out = Vec::new();
        out.try_reserve_exact(wrote)
            .map_err(|_| Failed(format!("`{symbol}` could not reserve {wrote} bytes")))?;
        out.resize(wrote, 0);
        let mut wrote = 0usize;
        // SAFETY: as above, with a buffer of the size the callee asked for.
        let code = unsafe {
            function(args.as_ptr(), args.len(), out.as_mut_ptr(), out.len(), &raw mut wrote)
        };
        if code == 1 {
            return Err(Failed(format!("`{symbol}` asked for more room twice")));
        }
        if code == 0 && wrote > out.len() {
            return Err(Failed(format!(
                "`{symbol}` claimed to write {wrote} bytes into a {}-byte buffer",
                out.len()
            )));
        }
        out.truncate(wrote.min(out.len()));
        Ok((code, out))
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        let handle = self.handle.get_mut().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: `handle` came from `dlopen` and is closed once, here.
        unsafe { dlclose(handle.0) };
    }
}
