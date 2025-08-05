//! Multisig instructions
use crate::{config::MultisigConfig, proposal::Proposal};
use solana_account_info::{next_account_info, AccountInfo};
use solana_msg::msg;
use solana_program_error::{ProgramError, ProgramResult};
use solana_pubkey::Pubkey;

/// initializes multisig write proposal.
pub fn initialize_multisig_write(accounts: &[AccountInfo<'_>], instr_data: &[u8]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let _payer = next_account_info(account_info_iter)?; // signer
    let proposal_account = next_account_info(account_info_iter)?; // writable
    let client_account = next_account_info(account_info_iter)?; // writable
    let multisig_account = next_account_info(account_info_iter)?; // read-only

    // Validate multisig config
    let _multisig = MultisigConfig::from_account_info(multisig_account)?;

    // Create the proposal state
    let proposal = Proposal {
        version: Proposal::CURRENT_VERSION,
        bump: 0,            // If not used with PDA, must be zero.
        instruction_tag: 5, // This is the instruction tag that needs to be run from the multisig given
        executed: 0,
        // PDA connected with the instruction we have to multisign by different parties
        client_account: *client_account.key,
        multisig_key: *multisig_account.key,
        // this is changed on the way
        signer_approvals: 0,
    };

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

    let mut proposal: Proposal = *bytemuck::from_bytes(&meta[..Proposal::SIZE]);

    if proposal.is_executed() {
        return Err(ProgramError::Custom(0)); // Already executed
    }

    if multisig_account.key != &proposal.multisig_key {
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
    {
        meta[..Proposal::SIZE].copy_from_slice(bytemuck::bytes_of(&proposal));
    }
    // If threshold reached, execute
    if proposal.is_ready_to_execute(multisig.threshold) {
        msg!("Threshold reached, executing instruction");

        pda_handler(payload, client_account, multisig_account.key)?;

        proposal.set_executed();

        meta[..Proposal::SIZE].copy_from_slice(bytemuck::bytes_of(&proposal));
    } else {
        msg!("Threshold not yet reached.");
    }

    Ok(())
}
