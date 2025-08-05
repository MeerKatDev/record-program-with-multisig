//! Multisig instructions
use crate::{config::MultisigConfig, proposal::Proposal};
use solana_account_info::{next_account_info, AccountInfo};
use solana_msg::msg;
use solana_program_error::{ProgramError, ProgramResult};
use solana_pubkey::Pubkey;

/// initializes multisig write proposal.
pub fn initialize_multisig_write(accounts: &[AccountInfo<'_>], instr_data: &[u8]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let payer = next_account_info(account_info_iter)?; // signer

    if !payer.is_signer {
        msg!("Payer is not a signer!");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let proposal_account = next_account_info(account_info_iter)?; // writable
    let client_account = next_account_info(account_info_iter)?; // writable

    if !proposal_account.is_writable {
        msg!("Proposal account must be writable");
        return Err(ProgramError::InvalidAccountData);
    }

    if !client_account.is_writable {
        msg!("Client account must be writable");
        return Err(ProgramError::InvalidAccountData);
    }

    let multisig_account = next_account_info(account_info_iter)?; // read-only

    if multisig_account.is_writable {
        msg!("Multisig account should be read-only");
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate multisig config
    let _multisig = MultisigConfig::from_account_info(multisig_account)?;

    let instruction_tag = instr_data
        .first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    // Create the proposal metadata
    let proposal = Proposal::new(
        *instruction_tag,
        *client_account.key,
        *multisig_account.key,
        instr_data,
    );

    // Proposal account should be large as metadata (struct data) + actual data
    let mut proposal_data = proposal_account.try_borrow_mut_data()?;
    let (meta, payload) = proposal_data.split_at_mut(Proposal::SIZE);

    let new_meta_bytes = bytemuck::bytes_of(&proposal);

    if new_meta_bytes.len() > meta.len() {
        return Err(ProgramError::InvalidInstructionData);
    }
    meta[..].copy_from_slice(new_meta_bytes);

    if instr_data.len() > payload.len() {
        return Err(ProgramError::InvalidInstructionData);
    }
    payload[..instr_data.len()].copy_from_slice(instr_data);

    Ok(())
}

/// Process approve. If threshold is reached, the write is executed.
pub fn process_approve_proposal<F>(accounts: &[AccountInfo<'_>], pda_handler: F) -> ProgramResult
where
    F: Fn(&[u8], &AccountInfo, &Pubkey) -> ProgramResult,
{
    let account_info_iter = &mut accounts.iter();
    let signer = next_account_info(account_info_iter)?; // signer
    let proposal_account = next_account_info(account_info_iter)?; // writable
    let client_account = next_account_info(account_info_iter)?; // writable
    let multisig_account = next_account_info(account_info_iter)?; // read-only

    let mut data = proposal_account.try_borrow_mut_data()?;

    if data.len() < Proposal::SIZE {
        msg!(
            "meta data is too small! data len: {}, proposal len: {}",
            data.len(),
            Proposal::SIZE
        );
        return Err(ProgramError::InvalidAccountData);
    }

    let (meta, payload) = data.split_at_mut(Proposal::SIZE);

    let mut proposal: Proposal = *bytemuck::try_from_bytes::<Proposal>(&meta[..Proposal::SIZE])
        .map_err(|e| {
            msg!("Invalid proposal deserialization: {:?}", e);
            ProgramError::InvalidArgument
        })?;

    if proposal.is_executed() {
        msg!("Proposal was already executed!");
        return Err(ProgramError::InvalidAccountData); // Already executed
    }

    if !proposal.is_instruction_data_correct(payload) {
        msg!("Invalid approving instruction data!");
        return Err(ProgramError::InvalidAccountData);
    }

    if multisig_account.key != &proposal.multisig_key {
        msg!("Multisignature accounts don't match!");
        return Err(ProgramError::InvalidArgument);
    }

    if client_account.key != &proposal.client_account {
        msg!("Client accounts don't match!");
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
        return Err(ProgramError::InvalidArgument);
    }

    // If threshold reached, execute
    if proposal.is_ready_to_execute(multisig.threshold) {
        msg!("Threshold reached, executing instruction");

        pda_handler(payload, client_account, multisig_account.key)?;

        proposal.set_executed();
    } else {
        msg!("Updating proposal, threshold not yet reached.");
    }

    meta[..Proposal::SIZE].copy_from_slice(bytemuck::bytes_of(&proposal));

    Ok(())
}
