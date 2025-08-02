//! Multisig configuration data
use solana_account_info::AccountInfo;
use solana_program_error::{ProgramError, ProgramResult};
use {
    bytemuck::{Pod, Zeroable},
    solana_program_pack::IsInitialized,
    solana_pubkey::Pubkey,
};

/// group size of signers
pub const MAX_SIGNERS: usize = 10;

/// Multisig config
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct MultisigConfig {
    /// Version for upgrade compatibility
    pub version: u8,
    /// Number of required approvals
    pub threshold: u8,
    /// Number of signers (must be <= MAX_SIGNERS)
    pub signer_count: u8,
    /// Signers list
    pub signers: [Pubkey; MAX_SIGNERS],
}

impl MultisigConfig {
    /// Current multisig version. Does not need to be aligned with proposal.
    pub const CURRENT_VERSION: u8 = 1;

    /// checks if the signer belongs to the group here
    pub fn is_signer(&self, key: &Pubkey) -> bool {
        self.signers[..self.signer_count as usize].contains(key)
    }

    /// derive config from its account info
    pub fn from_account_info(account_info: &AccountInfo) -> Result<Self, ProgramError> {
        let data = account_info.try_borrow_data()?;
        if data.len() < std::mem::size_of::<Self>() {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(*bytemuck::from_bytes::<Self>(
            &data[..std::mem::size_of::<Self>()],
        ))
    }

    /// verifies that all signatures are in place
    /// if not, an error is thrown.
    pub fn verify_signatures(&self, signers: &[AccountInfo]) -> ProgramResult {
        let mut unique_signer_keys = std::collections::HashSet::new();
        let mut approved_count = 0;

        for signer_info in signers {
            let key = signer_info.key;

            // Skip duplicates
            if unique_signer_keys.contains(key) {
                continue;
            }

            // Must be a configured signer and have signed the transaction
            if self.is_signer(key) && signer_info.is_signer {
                unique_signer_keys.insert(key);
                approved_count += 1;
            }
        }

        if approved_count < self.threshold as usize {
            return Err(ProgramError::MissingRequiredSignature);
        }

        Ok(())
    }
}

impl IsInitialized for MultisigConfig {
    fn is_initialized(&self) -> bool {
        self.version == Self::CURRENT_VERSION && self.signer_count <= MAX_SIGNERS as u8
    }
}
