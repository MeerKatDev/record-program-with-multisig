//! mod for multisig
pub mod config;
pub mod instructions;
pub mod proposal;

use config::MultisigConfig;
use solana_account_info::AccountInfo;
use solana_program_error::{ProgramError, ProgramResult};

/// verify signatures taking account info
/// avoids use of imports in the main program
pub fn verify_signatures(
    multisig_account: &AccountInfo,
    signer_infos: &[AccountInfo],
) -> ProgramResult {
    // Deserialize Multisig
    let multisig = MultisigConfig::from_account_info(multisig_account)?;
    if multisig.version != MultisigConfig::CURRENT_VERSION {
        return Err(ProgramError::InvalidAccountData);
    }
    // Verify enough signers
    multisig.verify_signatures(signer_infos)
}
