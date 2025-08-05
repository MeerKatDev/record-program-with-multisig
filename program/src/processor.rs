//! Program state processor

use std::mem::size_of;

use multisig::instructions::*;

use {
    crate::{error::RecordError, instruction::RecordInstruction, state::RecordData},
    solana_account_info::{next_account_info, AccountInfo},
    solana_msg::msg,
    solana_program_error::{ProgramError, ProgramResult},
    solana_program_pack::IsInitialized,
    solana_pubkey::Pubkey,
};

fn check_authority(authority_info: &AccountInfo, expected_authority: &Pubkey) -> ProgramResult {
    if expected_authority != authority_info.key {
        msg!("Incorrect record authority provided");
        return Err(RecordError::IncorrectAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("Record authority signature missing");
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

/// callback for multisig
/// This should have only instruction-related logic
pub fn multisig_handler(
    proposal_data: &[u8],
    client_account: &AccountInfo,
    multisig_key: &Pubkey,
) -> ProgramResult {
    let dest_data = &mut client_account.try_borrow_mut_data()?;

    let (offset, data_to_write): (usize, &[u8]) = match RecordInstruction::unpack(proposal_data) {
        Ok(RecordInstruction::ProposeMultiWrite { offset, data }) => (offset as usize, data),
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let start = offset + RecordData::WRITABLE_START_INDEX;
    let end = start + data_to_write.len();

    if end > dest_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }

    let header_meta = bytemuck::from_bytes::<RecordData>(&dest_data[..size_of::<RecordData>()]);

    if !header_meta.is_initialized() {
        return Err(ProgramError::UninitializedAccount);
    }

    if header_meta.authority != *multisig_key {
        return Err(ProgramError::IllegalOwner);
    }

    dest_data[start..end].copy_from_slice(data_to_write);

    Ok(())
}

/// Instruction processor
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = RecordInstruction::unpack(input)?;
    let account_info_iter = &mut accounts.iter();

    match instruction {
        RecordInstruction::Initialize => {
            msg!("RecordInstruction::Initialize");

            let data_info = next_account_info(account_info_iter)?;
            let authority_info = next_account_info(account_info_iter)?;

            let raw_data = &mut data_info.data.borrow_mut();
            if raw_data.len() < RecordData::WRITABLE_START_INDEX {
                return Err(ProgramError::InvalidAccountData);
            }

            let account_data = bytemuck::try_from_bytes_mut::<RecordData>(
                &mut raw_data[..RecordData::WRITABLE_START_INDEX],
            )
            .map_err(|_| ProgramError::InvalidArgument)?;
            if account_data.is_initialized() {
                msg!("Record account already initialized");
                return Err(ProgramError::AccountAlreadyInitialized);
            }

            account_data.authority = *authority_info.key;
            account_data.version = RecordData::CURRENT_VERSION;
            Ok(())
        }

        RecordInstruction::Write { offset, data } => {
            msg!("RecordInstruction::Write");
            let data_info = next_account_info(account_info_iter)?;
            let authority_info = next_account_info(account_info_iter)?;
            {
                let raw_data = &data_info.data.borrow();
                if raw_data.len() < RecordData::WRITABLE_START_INDEX {
                    return Err(ProgramError::InvalidAccountData);
                }
                let account_data = bytemuck::try_from_bytes::<RecordData>(
                    &raw_data[..RecordData::WRITABLE_START_INDEX],
                )
                .map_err(|_| ProgramError::InvalidArgument)?;
                if !account_data.is_initialized() {
                    msg!("Record account not initialized");
                    return Err(ProgramError::UninitializedAccount);
                }
                check_authority(authority_info, &account_data.authority)?;
            }
            let start = RecordData::WRITABLE_START_INDEX.saturating_add(offset as usize);
            let end = start.saturating_add(data.len());
            if end > data_info.data.borrow().len() {
                Err(ProgramError::AccountDataTooSmall)
            } else {
                data_info.data.borrow_mut()[start..end].copy_from_slice(data);
                Ok(())
            }
        }

        RecordInstruction::SetAuthority => {
            msg!("RecordInstruction::SetAuthority");
            let data_info = next_account_info(account_info_iter)?;
            let authority_info = next_account_info(account_info_iter)?;
            let new_authority_info = next_account_info(account_info_iter)?;
            let raw_data = &mut data_info.data.borrow_mut();
            if raw_data.len() < RecordData::WRITABLE_START_INDEX {
                return Err(ProgramError::InvalidAccountData);
            }
            let account_data = bytemuck::try_from_bytes_mut::<RecordData>(
                &mut raw_data[..RecordData::WRITABLE_START_INDEX],
            )
            .map_err(|_| ProgramError::InvalidArgument)?;
            if !account_data.is_initialized() {
                msg!("Record account not initialized");
                return Err(ProgramError::UninitializedAccount);
            }
            check_authority(authority_info, &account_data.authority)?;
            account_data.authority = *new_authority_info.key;
            Ok(())
        }

        RecordInstruction::CloseAccount => {
            msg!("RecordInstruction::CloseAccount");
            let data_info = next_account_info(account_info_iter)?;
            let authority_info = next_account_info(account_info_iter)?;
            let destination_info = next_account_info(account_info_iter)?;
            let raw_data = &mut data_info.data.borrow_mut();
            if raw_data.len() < RecordData::WRITABLE_START_INDEX {
                return Err(ProgramError::InvalidAccountData);
            }
            let account_data = bytemuck::try_from_bytes_mut::<RecordData>(
                &mut raw_data[..RecordData::WRITABLE_START_INDEX],
            )
            .map_err(|_| ProgramError::InvalidArgument)?;
            if !account_data.is_initialized() {
                msg!("Record not initialized");
                return Err(ProgramError::UninitializedAccount);
            }
            check_authority(authority_info, &account_data.authority)?;
            let destination_starting_lamports = destination_info.lamports();
            let data_lamports = data_info.lamports();
            **data_info.lamports.borrow_mut() = 0;
            **destination_info.lamports.borrow_mut() = destination_starting_lamports
                .checked_add(data_lamports)
                .ok_or(RecordError::Overflow)?;
            Ok(())
        }

        RecordInstruction::Reallocate { data_length } => {
            msg!("RecordInstruction::Reallocate");
            let data_info = next_account_info(account_info_iter)?;
            let authority_info = next_account_info(account_info_iter)?;

            {
                let raw_data = &mut data_info.data.borrow_mut();
                if raw_data.len() < RecordData::WRITABLE_START_INDEX {
                    return Err(ProgramError::InvalidAccountData);
                }
                let account_data = bytemuck::try_from_bytes::<RecordData>(
                    &raw_data[..RecordData::WRITABLE_START_INDEX],
                )
                .map_err(|_| ProgramError::InvalidArgument)?;
                if !account_data.is_initialized() {
                    msg!("Record not initialized");
                    return Err(ProgramError::UninitializedAccount);
                }
                check_authority(authority_info, &account_data.authority)?;
            }

            // needed account length is the sum of the meta data length and the specified
            // data length
            let needed_account_length = std::mem::size_of::<RecordData>()
                .checked_add(
                    usize::try_from(data_length).map_err(|_| ProgramError::InvalidArgument)?,
                )
                .unwrap();

            // reallocate
            if data_info.data_len() >= needed_account_length {
                msg!("no additional reallocation needed");
                return Ok(());
            }
            msg!(
                "reallocating +{:?} bytes",
                needed_account_length
                    .checked_sub(data_info.data_len())
                    .unwrap(),
            );
            data_info.resize(needed_account_length)?;
            Ok(())
        }
        RecordInstruction::ProposeMultiWrite { .. } => initialize_multisig_write(accounts, &instruction.pack()),

        RecordInstruction::ApproveProposal => process_approve_proposal(accounts, multisig_handler),
    }
}
