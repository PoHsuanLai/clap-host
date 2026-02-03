//! CLAP host implementation.
//!
//! Provides the host-side callbacks that plugins use to communicate
//! with the host application.

use clap_sys::host::clap_host;
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::version::CLAP_VERSION;
use std::ffi::c_void;
use std::ptr;

/// CLAP host implementation.
///
/// This struct provides the callbacks that plugins use to request
/// services from the host (restart, process, extensions, etc.).
pub struct ClapHost {
    inner: clap_host,
}

impl ClapHost {
    /// Create a new CLAP host with the given name and version.
    pub fn new(name: &'static str, vendor: &'static str, url: &'static str, version: &'static str) -> Self {
        // These need to be null-terminated C strings
        // For simplicity, we use static strings that are already null-terminated
        Self {
            inner: clap_host {
                clap_version: CLAP_VERSION,
                host_data: ptr::null_mut(),
                name: name.as_ptr() as *const i8,
                vendor: vendor.as_ptr() as *const i8,
                url: url.as_ptr() as *const i8,
                version: version.as_ptr() as *const i8,
                get_extension: Some(host_get_extension),
                request_restart: Some(host_request_restart),
                request_process: Some(host_request_process),
                request_callback: Some(host_request_callback),
            },
        }
    }

    /// Create a default CLAP host.
    pub fn default_host() -> Self {
        Self {
            inner: clap_host {
                clap_version: CLAP_VERSION,
                host_data: ptr::null_mut(),
                name: c"clap-host".as_ptr(),
                vendor: c"Rust".as_ptr(),
                url: c"https://github.com/AcaciaAudio/clap-host".as_ptr(),
                version: c"0.1.0".as_ptr(),
                get_extension: Some(host_get_extension),
                request_restart: Some(host_request_restart),
                request_process: Some(host_request_process),
                request_callback: Some(host_request_callback),
            },
        }
    }

    /// Get the raw clap_host pointer.
    pub fn as_raw(&self) -> *const clap_host {
        &self.inner
    }
}

impl Default for ClapHost {
    fn default() -> Self {
        Self::default_host()
    }
}

// Host callback implementations

unsafe extern "C" fn host_get_extension(
    _host: *const clap_host,
    _extension_id: *const i8,
) -> *const c_void {
    // TODO: Implement host extensions (log, thread-check, etc.)
    ptr::null()
}

unsafe extern "C" fn host_request_restart(_host: *const clap_host) {
    // TODO: Handle restart request
}

unsafe extern "C" fn host_request_process(_host: *const clap_host) {
    // TODO: Handle process request
}

unsafe extern "C" fn host_request_callback(_host: *const clap_host) {
    // TODO: Handle callback request
}

/// Output stream for saving plugin state.
pub struct OutputStream {
    buffer: Vec<u8>,
    stream: clap_ostream,
}

impl OutputStream {
    /// Create a new output stream.
    pub fn new() -> Self {
        let mut s = Self {
            buffer: Vec::new(),
            stream: clap_ostream {
                ctx: ptr::null_mut(),
                write: Some(ostream_write),
            },
        };
        s.stream.ctx = &mut s.buffer as *mut Vec<u8> as *mut c_void;
        s
    }

    /// Get the raw clap_ostream pointer.
    pub fn as_raw(&self) -> *const clap_ostream {
        &self.stream
    }

    /// Get the written data.
    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    /// Take the written data, consuming the stream.
    pub fn into_data(self) -> Vec<u8> {
        self.buffer
    }
}

impl Default for OutputStream {
    fn default() -> Self {
        Self::new()
    }
}

unsafe extern "C" fn ostream_write(
    stream: *const clap_ostream,
    buffer: *const c_void,
    size: u64,
) -> i64 {
    let out_buffer = &mut *((*stream).ctx as *mut Vec<u8>);
    let data = std::slice::from_raw_parts(buffer as *const u8, size as usize);
    out_buffer.extend_from_slice(data);
    size as i64
}

/// Input stream for loading plugin state.
pub struct InputStream<'a> {
    data: &'a [u8],
    position: usize,
    stream: clap_istream,
}

impl<'a> InputStream<'a> {
    /// Create a new input stream from data.
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            stream: clap_istream {
                ctx: ptr::null_mut(),
                read: Some(istream_read),
            },
        }
    }

    /// Get the raw clap_istream pointer.
    ///
    /// # Safety
    /// The returned pointer is only valid for the lifetime of this InputStream.
    pub fn as_raw(&mut self) -> *const clap_istream {
        // We need to set ctx to point to self
        self.stream.ctx = self as *mut InputStream as *mut c_void;
        &self.stream
    }

    /// Get the current position.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Get the remaining bytes.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }
}

unsafe extern "C" fn istream_read(stream: *const clap_istream, buffer: *mut c_void, size: u64) -> i64 {
    let input = &mut *((*stream).ctx as *mut InputStream);
    let remaining = input.data.len() - input.position;
    let to_read = (size as usize).min(remaining);

    if to_read == 0 {
        return 0;
    }

    let source = &input.data[input.position..input.position + to_read];
    let dest = std::slice::from_raw_parts_mut(buffer as *mut u8, to_read);
    dest.copy_from_slice(source);

    input.position += to_read;
    to_read as i64
}
