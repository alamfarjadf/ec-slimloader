#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::lifecycle::{
    load_firmware_version_from_cfpa, load_image_key_revocation_from_cfpa, load_lifecycle_from_cfpa,
    load_pqc_rotkh_from_cmpa, load_root_key_revocation_from_cfpa, load_rotk_usage_from_cmpa, load_rotkh_from_cmpa,
};
use crate::rom_api::{
    nboot_bool_is_true, NbootBool, NbootBoolValue, NbootCtx, NbootImgAuthParms, NbootLifecycleState,
    NbootRootKeyRevocation, NbootRootKeyType, NbootRootKeyUsage, NbootRotAuthParms, NbootStatus, RomApi,
};

const DEFAULT_NBOOT_PARMS: NbootImgAuthParms = NbootImgAuthParms {
    soc_RoTNVM: NbootRotAuthParms {
        soc_rootKeyRevocation: [
            NbootRootKeyRevocation::Revoked as u32,
            NbootRootKeyRevocation::Revoked as u32,
            NbootRootKeyRevocation::Revoked as u32,
            NbootRootKeyRevocation::Revoked as u32,
            //Start as revoked by default for safety; will be updated with real values from CFPA if read is successful.
            // This way if CFPA read fails for some reason, we won't accidentally treat revoked keys as valid.
        ],
        soc_imageKeyRevocation: 0xFFFF_FFFF, //Image key revocation use case: None? Still set highest revocation value by default for safety; will be updated with real value from CFPA if read is successful.
        soc_rkh: [0; 12],
        soc_rkh_1: [0; 12],      // PQC hash for hybrid keys
        soc_numberOfRootKeys: 4, // TODO: Must equal 4 per NXP example code.
        soc_rootKeyUsage: [
            NbootRootKeyUsage::Unused as u32,
            NbootRootKeyUsage::Unused as u32,
            NbootRootKeyUsage::Unused as u32,
            NbootRootKeyUsage::Unused as u32,
            // Start as unused by default for safety; will be updated with real values from CMPA if read is successful.
        ],
        soc_rootKeyTypeAndLength: NbootRootKeyType::EcdsaP384Mldsa87 as u32, //FIXED TO THIS because we are CNSA 2.0 compliant.
        soc_lifecycle: NbootLifecycleState::InField.nboot_soc_lifecycle(), // default to INFIELD (strict start), gets updated with real one further below.
    },
    soc_trustedFirmwareVersion: 0xFFFF_FFFF, // default to max version to be safe (any real version should be lower), gets updated with real one from CFPA further below
};

fn load_nboot_auth_parms_from_ifr() -> Result<NbootImgAuthParms, ec_slimloader::BootError> {
    let mut parms = DEFAULT_NBOOT_PARMS;

    if let Some(cmpa_rotkh) = load_rotkh_from_cmpa() {
        parms.soc_RoTNVM.soc_rkh = cmpa_rotkh;
        defmt_or_log::trace!("RKTH loaded from CMPA");
    } else {
        defmt_or_log::warn!("CMPA ROTKH read failed");
        return Err(ec_slimloader::BootError::RootOfTrust);
    }

    // Load PQC ROTKH for hybrid keys
    if let Some(cmpa_pqc_rotkh) = load_pqc_rotkh_from_cmpa() {
        parms.soc_RoTNVM.soc_rkh_1 = cmpa_pqc_rotkh;
        defmt_or_log::trace!("PQC RKTH loaded from CMPA");
    } else {
        defmt_or_log::warn!("CMPA PQC ROTKH read failed");
        return Err(ec_slimloader::BootError::RootOfTrust);
    }

    //Load additional lifecycle state from CFPA/CMPA
    if let Some(cfpa_img_key_revocation) = load_image_key_revocation_from_cfpa() {
        parms.soc_RoTNVM.soc_imageKeyRevocation = cfpa_img_key_revocation;
    }

    if let Some(cfpa_root_key_revocation) = load_root_key_revocation_from_cfpa() {
        parms.soc_RoTNVM.soc_rootKeyRevocation = cfpa_root_key_revocation.map(|r| r as u32);
    }

    if let Some(cfpa_fw_version) = load_firmware_version_from_cfpa() {
        parms.soc_trustedFirmwareVersion = cfpa_fw_version;
    }

    if let Some(cmpa_root_key_usage) = load_rotk_usage_from_cmpa() {
        parms.soc_RoTNVM.soc_rootKeyUsage = cmpa_root_key_usage.map(|u| u as u32);
    }

    if let Some(cfpa_lifecycle) = load_lifecycle_from_cfpa() {
        parms.soc_RoTNVM.soc_lifecycle = cfpa_lifecycle.nboot_soc_lifecycle();
    }

    Ok(parms)
}

/// Verify the authenticity of the image at the given base address using the NBOOT ROM API. This includes initializing the NBOOT context, loading lifecycle and root of trust information from CFPA/CMPA,
/// deriving the image RKTH from the AHAB container, and calling nboot_img_authenticate_romapi. Returns Ok(()) if authentication is successful, or an appropriate BootError if any step fails or if authentication fails.
/// Will ONLY authenticate if CMPA secure boot settings is configured correctly, correct key set (as established by the ROTKH values) is used for signing, and the image is properly signed as an HYBRID (ECDSA + ML-DSA) image.
/// In dev mode, if the RKTH derived from the image does not match the ROTKH in CMPA, it will be copied to the ROTKH to allow authentication to proceed (this allows flexibility in dev mode since keys may not be provisioned yet),
/// but in production mode, a mismatch will cause authentication to fail (to prevent unauthorized images from being authenticated).
pub fn verify_authenticity<'d>(image_base: *const u8) -> Result<(), ec_slimloader::BootError> {
    let mut parms = load_nboot_auth_parms_from_ifr()?;

    let n_boot_api = RomApi::get().nboot();
    let mut ctx: NbootCtx = unsafe { core::mem::zeroed() };
    let mut sig_ok: NbootBool = NbootBoolValue::False as u32;

    defmt_or_log::trace!("Initializing NBOOT context");
    let context_init_status = n_boot_api.nboot_context_init(&mut ctx);
    if context_init_status != NbootStatus::Success {
        return Err(ec_slimloader::BootError::Authenticate);
    }

    defmt_or_log::trace!("begin auth");
    let status = n_boot_api.nboot_img_authenticate_romapi(&mut ctx, image_base, &mut sig_ok, &mut parms);

    n_boot_api.nboot_context_deinit(&mut ctx);

    match (status, sig_ok) {
        (NbootStatus::Success, s) if nboot_bool_is_true(s) => {
            defmt_or_log::info!("Hybrid Auth OK");
            Ok(())
        }
        (status, _) => {
            let boot_error = status.into();

            defmt_or_log::error!("Auth failed with status {:?}: {:?}", status, boot_error);
            Err(boot_error)
        }
    }
}
