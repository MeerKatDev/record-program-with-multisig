//! Multisig proposal data
use {
    bytemuck::{Pod, Zeroable},
    solana_keccak_hasher::hash,
    solana_program_pack::IsInitialized,
    solana_pubkey::Pubkey,
};

/// A pending instruction proposal for multisig-controlled actions
/// Instruction data is not included, as it will be included in the account.
/// This structure represents metadata.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Proposal {
    /// Struct version
    pub version: u8,
    /// Has the proposal been executed: 0 - false, 1 - true
    pub executed: u8,
    /// Single-digit discriminator for instruction to be executed
    pub instruction_tag: u8,
    /// Bitmask of approvals (up to 16 signers)
    pub signer_approvals: u16,
    /// Account being targeted
    pub client_account: Pubkey,
    /// Multisig account controlling the proposal
    pub multisig_key: Pubkey,
    /// Data hash
    pub hashed_data: [u8; 32],
}

impl Proposal {
    /// Current proposal version.
    pub const CURRENT_VERSION: u8 = 1;

    /// Offset in account data where `data` payload begins
    /// 1 + 1 + 1 + 2 + 32 + 32 + 32
    pub const SIZE: usize = 101;

    pub fn new(
        instruction_tag: u8,
        client_account: Pubkey,
        multisig_key: Pubkey,
        instr_data: &[u8],
    ) -> Self {
        let hashed_data = hash(Self::trim_trailing_zeros_slice(instr_data)).0;

        Self {
            version: Self::CURRENT_VERSION,
            executed: 0,
            signer_approvals: 0,
            instruction_tag,
            client_account,
            multisig_key,
            hashed_data,
        }
    }

    fn trim_trailing_zeros_slice(data: &[u8]) -> &[u8] {
        let mut end = data.len();
        while end > 0 && data[end - 1] == 0 {
            end -= 1;
        }
        &data[..end]
    }

    pub fn is_instruction_data_correct(&self, instr_data: &[u8]) -> bool {
        let hashed_data = hash(Self::trim_trailing_zeros_slice(instr_data)).0;
        self.hashed_data == hashed_data
    }

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
    /// if it's already one, it means that someone is manipulating things
    pub fn set_executed(&mut self) {
        if self.executed == 1 {
            panic!("Bad error! This should never ever happen!");
        }
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
