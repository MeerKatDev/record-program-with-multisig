//! Multisig instructions
use crate::multisig::{config::MultisigConfig, proposal::Proposal};
use crate::state::RecordData;

use solana_account_info::{next_account_info, AccountInfo};
use solana_msg::msg;
use solana_program_error::{ProgramError, ProgramResult};
use solana_program_pack::IsInitialized;

/// initializes multisig write proposal.
pub fn initialize_multisig_write(
    accounts: &[AccountInfo<'_>],
    offset: u64,
    data: &[u8],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let _payer = next_account_info(account_info_iter)?; // signer
    let proposal_account = next_account_info(account_info_iter)?; // writable
    let record_account = next_account_info(account_info_iter)?; // writable
    let multisig_account = next_account_info(account_info_iter)?; // read-only

    // Validate multisig config
    let _multisig = MultisigConfig::from_account_info(multisig_account)?;

    // Create the proposal state
    let proposal = Proposal {
        version: Proposal::CURRENT_VERSION,
        bump: 0,            // if not used with PDA, must be zero.
        instruction_tag: 5, // 1 = Write
        executed: 0,
        record_account: *record_account.key,
        multisig: *multisig_account.key,
        signer_approvals: 0,
        offset,
        data_length: data.len() as u32,
    };

    // Write to the proposal account
    let mut proposal_data = proposal_account.try_borrow_mut_data()?;
    let (meta, payload) = proposal_data.split_at_mut(Proposal::DATA_START_INDEX);
    meta[..std::mem::size_of::<Proposal>()].copy_from_slice(bytemuck::bytes_of(&proposal));
    payload[..data.len()].copy_from_slice(data);

    Ok(())
}

/// Process approve. If threshold is reached, the write is executed.
pub fn process_approve_proposal(accounts: &[AccountInfo<'_>]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let signer = next_account_info(account_info_iter)?; // signer
    let proposal_account = next_account_info(account_info_iter)?; // writable
    let record_account = next_account_info(account_info_iter)?; // writable
    let multisig_account = next_account_info(account_info_iter)?; // read-only

    // Load proposal
    let mut data = proposal_account.try_borrow_mut_data()?;
    let (meta, payload) = data.split_at_mut(Proposal::DATA_START_INDEX);

    if meta.len() <= std::mem::size_of::<Proposal>() {
        return Err(ProgramError::InvalidAccountData);
    }
    let mut proposal: Proposal = *bytemuck::from_bytes(&meta[..std::mem::size_of::<Proposal>()]);

    if proposal.is_executed() {
        return Err(ProgramError::Custom(0)); // Already executed
    }

    if multisig_account.key != &proposal.multisig {
        return Err(ProgramError::InvalidArgument);
    }

    // Load multisig config
    let multisig = MultisigConfig::from_account_info(multisig_account)?;

    // Determine signer index
    let signer_index = multisig
        .signers
        .iter()
        .position(|k| k == signer.key)
        .ok_or(ProgramError::MissingRequiredSignature)?;

    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let updated = proposal.approve(signer_index);
    if !updated {
        msg!("Signer already approved");
        return Ok(());
    }

    // Write back updated state
    meta[..std::mem::size_of::<Proposal>()].copy_from_slice(bytemuck::bytes_of(&proposal));

    // If threshold reached, execute
    if proposal.is_ready_to_execute(multisig.threshold) {
        msg!("Threshold reached, executing instruction");

        if proposal.instruction_tag == 5 {
            let record_data = &mut record_account.try_borrow_mut_data()?;
            let header = bytemuck::from_bytes_mut::<RecordData>(&mut record_data[..33]);
            if !header.is_initialized() {
                return Err(ProgramError::UninitializedAccount);
            }
            if header.authority != *multisig_account.key {
                return Err(ProgramError::IllegalOwner);
            }

            let offset = proposal.offset as usize;
            let start = RecordData::WRITABLE_START_INDEX + offset;
            let end = start + proposal.data_length as usize;

            if end > record_data.len() {
                return Err(ProgramError::InvalidAccountData);
            }

            record_data[start..end].copy_from_slice(&payload[..proposal.data_length as usize]);
            proposal.set_executed();

            // Write proposal back with executed = true
            meta[..std::mem::size_of::<Proposal>()].copy_from_slice(bytemuck::bytes_of(&proposal));
        } else {
            return Err(ProgramError::InvalidInstructionData); // unsupported instruction
        }
    }

    Ok(())
}
