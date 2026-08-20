#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
use core::ffi::c_char;

mod flash;
mod flexspi_nor;
mod kb;
mod nboot;
mod spi_flash;

pub use flash::*;
pub use flexspi_nor::*;
pub use kb::*;
pub use nboot::*;
pub use spi_flash::*;

use self::flash::FlashDriverRaw;
use self::flexspi_nor::FlexspiNorFlashDriverRaw;
use self::kb::KBApiDriverRaw;
use self::nboot::NbootDriverRaw;
use self::spi_flash::SpiFlashDriverRaw;

pub type Status = u32;
pub type NbootBool = u32;
pub type NbootStatusProtected = u64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StandardVersionFields {
    pub bugfix: u8,
    pub minor: u8,
    pub major: u8,
    pub name: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union StandardVersion {
    pub fields: StandardVersionFields,
    pub version: u32,
}

#[repr(C)]
struct RomApiRaw {
    // NXP usage: uint32_t arg = ...; g_bootloaderTree->runBootloader(&arg);
    // The ROM API takes a pointer to the argument word (NULL is allowed for default behavior).
    pub run_bootloader: unsafe extern "C" fn(arg: *const u32),
    // Flash driver interface table.
    pub flash_api: *const FlashDriverRaw,
    pub kb_api: *const KBApiDriverRaw,
    pub nboot_api: *const NbootDriverRaw,
    pub flex_spi_api: *const FlexspiNorFlashDriverRaw,
    pub spi_flash_api: *const SpiFlashDriverRaw,
    pub version: StandardVersion,
    pub copyright: *const c_char,
}

#[derive(Clone, Copy)]
pub struct RomApi {
    raw: &'static RomApiRaw,
}

impl RomApi {
    const fn from_raw(raw: &'static RomApiRaw) -> Self {
        Self { raw }
    }

    pub fn run_bootloader(&self, arg: *const u32) {
        unsafe { (self.raw.run_bootloader)(arg) }
    }

    pub fn flash_api(&self) -> FlashDriver {
        unsafe { FlashDriver::from_raw(&*self.raw.flash_api) }
    }

    pub fn kb_api(&self) -> KBApiDriver {
        unsafe { KBApiDriver::from_raw(&*self.raw.kb_api) }
    }

    pub fn nboot_api(&self) -> NbootDriver {
        unsafe { NbootDriver::from_raw(&*self.raw.nboot_api) }
    }

    pub fn flex_spi_api(&self) -> FlexspiNorFlashDriver {
        unsafe { FlexspiNorFlashDriver::from_raw(&*self.raw.flex_spi_api) }
    }

    pub fn spi_flash_api(&self) -> SpiFlashDriver {
        unsafe { SpiFlashDriver::from_raw(&*self.raw.spi_flash_api) }
    }

    pub fn version(&self) -> StandardVersion {
        self.raw.version
    }

    pub fn copyright(&self) -> *const c_char {
        self.raw.copyright
    }
}

pub type BootloaderTree = RomApi;

pub fn rom_api() -> RomApi {
    const ROM_API_BASE: usize = 0x1303_D800; // from MCXA Reference Manual.
    unsafe {
        let ptr = ROM_API_BASE as *const RomApiRaw;
        RomApi::from_raw(&*ptr)
    }
}

pub fn bootloader_tree() -> BootloaderTree {
    rom_api()
}

// runBootloader API fields (Table 31)
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootTag {
    EnterBoot = 0xEB << 24,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootMode {
    PrimaryMasterBoot = 0x0 << 20,
    IspBoot = 0x1 << 20,
    ProvFwMode = 0x2 << 20,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootIspInterface {
    AutoDetection = 0x0 << 16,
    Uart = 0x1 << 16,
    Spi = 0x2 << 16,
    I2c = 0x8 << 16,
    UsbHid = 0x10 << 16,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootMasterFlashBootOption {
    InternalFlash = 0x0 << 16,
    FlexspiFlash = 0x2 << 16,
    OneBitSpiNorFlash = 0x3 << 16,
    AutoDetection = 0x1F << 16,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootInterfaceInstance {
    FlexspiPortA = 0x0 << 12,
    FlexspiPortB = 0x1 << 12,
    FlexspiPortAAndB = 0x2 << 12,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootImageIndex {
    Image0 = 0x0 << 8,
    Image1 = 0x1 << 8,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootRecoveryBootCfg1 {
    SpiNorBaudRate0 = 0x0 << 6,
    SpiNorBaudRate1 = 0x1 << 6,
    SpiNorBaudRate2 = 0x2 << 6,
    SpiNorBaudRate3 = 0x3 << 6,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootRecoveryBootCfg0 {
    SpiNorChipSelect0 = 0x0 << 4,
    SpiNorChipSelect1 = 0x1 << 4,
    SpiNorChipSelect2 = 0x2 << 4,
    SpiNorChipSelect3 = 0x3 << 4,
}

/// Helper function to invoke the ROM API's run_bootloader function with the appropriate argument to enter ISP mode over UART. This can be used as a fallback if the main bootloader fails and we want to recover by flashing over UART using NXP's ISP tools.
/// The function will not return since the bootloader will take over execution after this call, but we still include an infinite loop after the call to satisfy the Rust type system since the function is declared to return ! (never).
pub fn run_bootloader_uart() -> ! {
    // Build arg: tag 0xEB, mode ISP(1), interface UART(1)
    let arg: u32 = RunBootTag::EnterBoot as u32 | RunBootMode::IspBoot as u32 | RunBootIspInterface::Uart as u32;
    bootloader_tree().run_bootloader(&arg as *const u32);
    loop {
        core::hint::spin_loop()
    }
}

/// Helper function to get a pointer to the flash driver API from the ROM API tree.
pub fn flash_driver() -> FlashDriver {
    // Match NXP usage: g_bootloaderTree->flashDriver->...
    // The bootloader tree stores a direct pointer to the flash driver interface.
    bootloader_tree().flash_api()
}

/// Helper function to get a pointer to the nboot API from the ROM API tree.
pub fn nboot() -> NbootDriver {
    // Match NXP usage: g_bootloaderTree->nbootDriver->...
    bootloader_tree().nboot_api()
}

/// Helper function to get a pointer to the KB driver API from the ROM API tree.
pub fn kb() -> KBApiDriver {
    bootloader_tree().kb_api()
}

/// Helper function to get a pointer to the FlexSPI NOR flash driver API from the ROM API tree.
pub fn flexspi_nor() -> FlexspiNorFlashDriver {
    bootloader_tree().flex_spi_api()
}

/// Helper function to get a pointer to the SPI flash driver API from the ROM API tree.
pub fn spi_flash() -> SpiFlashDriver {
    bootloader_tree().spi_flash_api()
}

/// Used to get the default FlashConfig struct to be inited by flash_init().
pub fn flash_cfg_for_rom_api() -> FlashConfig {
    FlashConfig {
        pflash_block_base: 0,
        pflash_total_size: 0,
        pflash_block_count: 0,
        pflash_page_size: 0,
        pflash_sector_size: 0,
        ffr_config: FlashFfrConfig {
            ffr_block_base: 0,
            ffr_total_size: 0,
            ffr_page_size: 0,
            sector_size: 0,
            cfpa_page_version: 0,
            cfpa_page_offset: 0,
        },
        mode_config: FlashModeConfig::new(
            0,
            FlashReadSingleWordConfig::new(
                FlashReadEccOption::On,
                FlashReadMarginOption::Normal,
                FlashReadDmaccOption::Disabled,
            ),
            FlashSetWriteModeConfig::new(FlashRampControlOption::Reserved, FlashRampControlOption::Reserved),
            FlashSetReadModeConfig::new(0, 0, 0),
        ),
        nboot_ctx: core::ptr::null_mut(),
        use_ahb_read: true,
    }
}

const KSTATUS_SUCCESS: u32 = 0;
const KSTATUS_FAIL: u32 = 1;
const KSTATUS_INVALID_ARGUMENT: u32 = 4;

const KSTATUS_FLASH_SUCCESS: u32 = 0;
const KSTATUS_FLASH_INVALID_ARGUMENT: u32 = 4;
const KSTATUS_FLASH_ALIGNMENT_ERROR: u32 = 101;
const KSTATUS_FLASH_ADDRESS_ERROR: u32 = 102;
const KSTATUS_FLASH_SIZE_ERROR: u32 = 100;
const KSTATUS_FLASH_COMMAND_FAILURE: u32 = 105;
const KSTATUS_FLASH_UNKNOWN_PROPERTY: u32 = 106;
const KSTATUS_FLASH_ERASE_KEY_ERROR: u32 = 107;
const KSTATUS_FLASH_REGION_EXECUTE_ONLY: u32 = 108;
const KSTATUS_FLASH_COMMAND_NOT_SUPPORTED: u32 = 111;
const KSTATUS_FLASH_READ_ONLY_PROPERTY: u32 = 112;
const KSTATUS_FLASH_INVALID_PROPERTY_VALUE: u32 = 113;
const KSTATUS_FLASH_ECC_ERROR: u32 = 116;
const KSTATUS_FLASH_COMPARE_ERROR: u32 = 117;
const KSTATUS_FLASH_INVALID_WAIT_STATE_CYCLES: u32 = 119;

// SPI flash driver status codes
const KSTATUS_SPIFLASH_SUCCESS: u32 = KSTATUS_SUCCESS;
const KSTATUS_SPIFLASH_FAIL: u32 = KSTATUS_FAIL;

// FlexSPI flash driver status codes
const KSTATUS_FLEXSPI_SUCCESS: u32 = KSTATUS_SUCCESS;
const KSTATUS_FLEXSPI_FAIL: u32 = KSTATUS_FAIL;
const KSTATUS_FLEXSPI_INVALID_ARGUMENT: u32 = KSTATUS_INVALID_ARGUMENT;
const KSTATUS_FLEXSPI_SEQUENCE_EXECUTION_TIMEOUT: u32 = 6000;
const KSTATUS_FLEXSPI_INVALID_SEQUENCE: u32 = 6001;
const KSTATUS_FLEXSPI_DEVICE_TIMEOUT: u32 = 6002;

const KSTATUS_FLEXSPINOR_PROGRAM_FAIL: u32 = 20100;
const KSTATUS_FLEXSPINOR_ERASE_SECTOR_FAIL: u32 = 20101;
const KSTATUS_FLEXSPINOR_ERASE_ALL_FAIL: u32 = 20102;
const KSTATUS_FLEXSPINOR_WAIT_TIMEOUT: u32 = 20103;
const KSTATUS_FLEXSPINOR_WRITE_ALIGNMENT_ERROR: u32 = 20105;
const KSTATUS_FLEXSPINOR_COMMAND_FAILURE: u32 = 20106;
const KSTATUS_FLEXSPINOR_SFDP_NOT_FOUND: u32 = 20107;
const KSTATUS_FLEXSPINOR_UNSUPPORTED_SFDP_VERSION: u32 = 20108;
const KSTATUS_FLEXSPINOR_FLASH_NOT_FOUND: u32 = 20109;
const KSTATUS_FLEXSPINOR_DTR_READ_DUMMY_PROBE_FAILED: u32 = 20110;

const KSTATUS_NBOOT_SUCCESS: u64 = 0x5A5A_5A5A;
const KSTATUS_NBOOT_FAIL: u64 = 0x5A5A_A5A5;
const KSTATUS_NBOOT_INVALID_ARGUMENT: u64 = 0x5A5A_A5F0;

// NBOOT API status codes (MCXA ROM, Table 46 / 9.2.5.11)
// These are returned by APIs such as `nboot_mem_crypt_range_checker`.
const KNBOOT_OPERATION_ALLOWED: u64 = 0x3C5A_33CC;
const KNBOOT_OPERATION_DISALLOWED: u64 = 0x5AA5_CC33;
const KSTATUS_NBOOT_KEY_NOT_AVAILABLE: u64 = 0x5A5A_A5E6;

const KSTATUS_ROMLDR_DATA_UNDERRUN: u32 = 10109;
const KSTATUS_ROMLDR_JUMP_RETURNED: u32 = 10110;
const KSTATUS_ROMLDR_ROLLBACK_BLOCKED: u32 = 10115;
const KSTATUS_ROMLDR_PENDING_JUMP_COMMAND: u32 = 10119;

// ROM API status codes
const KSTATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH: u32 = 10802;
const KSTATUS_ROM_API_INVALID_BUFFER: u32 = 10803;

// KBoot (KB) status codes (Table 35); KB reuses generic/ROM loader/ROM API status space; these aliases make callsites clearer.
const KSTATUS_KB_SUCCESS: u32 = KSTATUS_SUCCESS;
const KSTATUS_KB_FAIL: u32 = KSTATUS_FAIL;
const KSTATUS_KB_INVALID_ARGUMENT: u32 = KSTATUS_INVALID_ARGUMENT;

const KSTATUS_KB_ROMLDR_DATA_UNDERRUN: u32 = KSTATUS_ROMLDR_DATA_UNDERRUN;
const KSTATUS_KB_ROMLDR_JUMP_RETURNED: u32 = KSTATUS_ROMLDR_JUMP_RETURNED;
const KSTATUS_KB_ROMLDR_ROLLBACK_BLOCKED: u32 = KSTATUS_ROMLDR_ROLLBACK_BLOCKED;
const KSTATUS_KB_ROMLDR_PENDING_JUMP_COMMAND: u32 = KSTATUS_ROMLDR_PENDING_JUMP_COMMAND;

const KSTATUS_KB_BUFFER_SIZE_NOT_ENOUGH: u32 = KSTATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH;
const KSTATUS_KB_INVALID_BUFFER: u32 = KSTATUS_ROM_API_INVALID_BUFFER;
