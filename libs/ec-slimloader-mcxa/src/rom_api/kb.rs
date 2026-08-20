use core::ffi::c_void;

use ec_slimloader::BootError;

use super::Status;

// KBoot (KB) ROM API

#[repr(C)]
pub struct KbRegion {
    // Region base address.
    pub address: u32,
    // Region length in bytes.
    pub length: u32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KbOperation {
    // Verify/authenticate image.
    AuthenticateImage = 1,
    // Load image.
    LoadImage = 2,
    // Number of KB operations.
    OperationCount = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KbLoadSb {
    // Profile selector (meaning per ROM header / implementation).
    pub profile: u32,
    // Minimum build number.
    pub minBuildNumber: u32,
    // Override SB boot section ID.
    pub overrideSBBootSectionID: u32,
    // User SB KEK pointer.
    pub userSBKEK: *mut u32,
    // Number of region descriptors.
    pub regionCount: u32,
    // Pointer to regions array.
    pub regions: *const KbRegion,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KbAuthenticate {
    // Profile selector (meaning per ROM header / implementation).
    pub profile: u32,
    // Minimum build number.
    pub minBuildNumber: u32,
    // Maximum image length.
    pub maxImageLength: u32,
    // User RHK pointer.
    pub userRHK: *mut u32,
}

#[repr(C)]
pub union KbOptionsParams {
    pub authenticate: KbAuthenticate,
    pub loadSB: KbLoadSb,
}

#[repr(C)]
pub struct KbOptions {
    // Must be KB_API_VERSION.
    pub version: u32,
    // Caller-provided buffer used by ROM.
    pub buffer: *mut u8,
    // Length of buffer in bytes.
    pub bufferLength: u32,
    // Requested operation.
    pub op: KbOperation,
    // Operation-specific parameters.
    pub params: KbOptionsParams,
}

#[repr(C)]
pub struct KbBufferDesc {
    // Buffer pointer.
    pub buf: *mut u8,
    // Buffer length in bytes.
    pub len: u32,
    // Allocated size.
    pub allocated: u32,
}

#[repr(C)]
pub struct KbSessionRef {
    // Options used to create the session.
    pub options: KbOptions,
    // Internal buffer descriptor.
    pub buffer_desc: KbBufferDesc,
    // Opaque operation context.
    pub op_context: *mut c_void,
}

#[repr(C)]
pub(super) struct KBApiDriverRaw {
    // Initialize KB session.
    pub kb_init: unsafe extern "C" fn(session: *mut *mut KbSessionRef, options: *const KbOptions) -> Status,
    // Deinitialize KB session.
    pub kb_deinit: unsafe extern "C" fn(session: *mut KbSessionRef) -> Status,
    // Execute KB operation over a data buffer.
    pub kb_execute: unsafe extern "C" fn(session: *mut KbSessionRef, data: *const u8, dataLength: u32) -> Status,
}

#[derive(Clone, Copy)]
pub struct KBApiDriver {
    raw: &'static KBApiDriverRaw,
}

impl KBApiDriver {
    pub(super) const fn from_raw(raw: &'static KBApiDriverRaw) -> Self {
        Self { raw }
    }

    pub fn kb_init(&self, session: *mut *mut KbSessionRef, options: *const KbOptions) -> KbStatus {
        unsafe { (self.raw.kb_init)(session, options) }.into()
    }

    pub fn kb_deinit(&self, session: *mut KbSessionRef) -> KbStatus {
        unsafe { (self.raw.kb_deinit)(session) }.into()
    }

    pub fn kb_execute(&self, session: *mut KbSessionRef, data: *const u8, dataLength: u32) -> KbStatus {
        unsafe { (self.raw.kb_execute)(session, data, dataLength) }.into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KbStatus {
    Success,
    Fail,
    InvalidArgument,
    RomLdrDataUnderrun,
    RomLdrJumpReturned,
    RomLdrRollbackBlocked,
    RomLdrPendingJumpCommand,
    RomApiBufferSizeNotEnough,
    RomApiInvalidBuffer,
    Unknown(u32),
}

impl From<u32> for KbStatus {
    fn from(raw: u32) -> Self {
        match raw {
            super::KSTATUS_KB_SUCCESS => Self::Success,
            super::KSTATUS_KB_FAIL => Self::Fail,
            super::KSTATUS_KB_INVALID_ARGUMENT => Self::InvalidArgument,
            super::KSTATUS_KB_ROMLDR_DATA_UNDERRUN => Self::RomLdrDataUnderrun,
            super::KSTATUS_KB_ROMLDR_JUMP_RETURNED => Self::RomLdrJumpReturned,
            super::KSTATUS_KB_ROMLDR_ROLLBACK_BLOCKED => Self::RomLdrRollbackBlocked,
            super::KSTATUS_KB_ROMLDR_PENDING_JUMP_COMMAND => Self::RomLdrPendingJumpCommand,
            super::KSTATUS_KB_BUFFER_SIZE_NOT_ENOUGH => Self::RomApiBufferSizeNotEnough,
            super::KSTATUS_KB_INVALID_BUFFER => Self::RomApiInvalidBuffer,
            other => Self::Unknown(other),
        }
    }
}

impl From<KbStatus> for BootError {
    fn from(status: KbStatus) -> Self {
        match status {
            KbStatus::InvalidArgument | KbStatus::RomApiBufferSizeNotEnough | KbStatus::RomApiInvalidBuffer => {
                BootError::Markers
            }
            KbStatus::Fail => BootError::IO,
            KbStatus::RomLdrRollbackBlocked => BootError::Markers,
            // Data underrun / pending-jump are usually flow control for streaming loaders,
            // but if surfaced as an error, treat as I/O.
            KbStatus::RomLdrDataUnderrun | KbStatus::RomLdrJumpReturned | KbStatus::RomLdrPendingJumpCommand => {
                BootError::IO
            }
            _ => BootError::IO,
        }
    }
}
