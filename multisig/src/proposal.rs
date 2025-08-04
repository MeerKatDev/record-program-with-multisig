//! Multisig proposal data
use {
    bytemuck::{Pod, Zeroable},
    solana_program_pack::IsInitialized,
    solana_pubkey::Pubkey,
};

/// A pending instruction proposal for multisig-controlled actions
// std mem gives 82?
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Proposal {
    /// Struct version
    pub version: u8,
    /// Proposal bump seed
    pub bump: u8,
    /// Executing instruction code (e.g., 1 for Write)
    pub instruction_tag: u8,
    /// Has the proposal been executed: 0 - false, 1 - true
    pub executed: u8,
    /// Account being targeted
    pub pda_account: Pubkey,
    /// Multisig account controlling the proposal
    pub multisig: Pubkey,
    /// Bitmask of approvals (up to 16 signers)
    pub signer_approvals: u16,
    /// Custom field: offset (used for Write)
    pub offset: u64,
    /// Custom field: data length
    pub data_length: u32,
}

impl Proposal {
    /// Current proposal version.
    pub const CURRENT_VERSION: u8 = 1;

    /// Offset in account data where `data` payload begins
    /// 1 + 1 + 1 + 1 + 32 + 32 + 2 + 8 + 4
    pub const SIZE: usize = 82;

    /// check is this signer already approved.
    pub fn is_approved_by(&self, signer_index: usize) -> bool {
        (self.signer_approvals & (1 << signer_index)) != 0
    }

    /// count approvals
    pub fn approval_count(&self) -> u8 {
        self.signer_approvals.count_ones() as u8
    }

    /// approve by signer. existence of this index has to be checked earlier.
    pub fn approve(&mut self, signer_index: usize) -> bool {
        if !self.is_approved_by(signer_index) {
            self.signer_approvals |= 1 << signer_index;
            true
        } else {
            false
        }
    }

    /// was this proposal executed already
    pub fn is_executed(&self) -> bool {
        self.executed != 0
    }

    /// set as executed
    pub fn set_executed(&mut self) {
        self.executed = 1;
    }

    /// can this proposal be executed
    pub fn is_ready_to_execute(&self, threshold: u8) -> bool {
        self.approval_count() >= threshold && !self.is_executed()
    }
}

impl IsInitialized for Proposal {
    fn is_initialized(&self) -> bool {
        self.version == Self::CURRENT_VERSION
    }
}
