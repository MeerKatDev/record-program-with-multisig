use all2all_controller::{
    processor::process_instruction, 
    state::RecordData
};
    
use multisig::{
    config::MultisigConfig,
    proposal::Proposal
};

use bytemuck::bytes_of;

use {
    solana_program_test::*,
    solana_pubkey::Pubkey,
    solana_sdk::{
        msg,
        account::Account,
        instruction::{AccountMeta, Instruction},
        signature::{Keypair, Signer},
        transaction::Transaction,
    },
};

#[tokio::test]
async fn test_multisig_write_approval_execution() {
    // === Setup Program Test Environment ===
    let program_id = Pubkey::new_unique();
    // setting up the client program
    let mut program_test = ProgramTest::new(
        "all2all_controller",
        program_id,
        processor!(process_instruction),
    );

    // === Create Multisig Config ===
    // create signers for the multisig session
    let signer1 = Keypair::new();
    let signer2 = Keypair::new();
    let signer3 = Keypair::new();
    let signers = [signer1.pubkey(), signer2.pubkey(), signer3.pubkey()];

    // PDA account for multisig
    let multisig_key = Pubkey::new_unique();
    // Setting up a multisig session
    let multisig_config = MultisigConfig::new(2, &signers).unwrap();

    program_test.add_account(
        multisig_key,
        Account {
            lamports: 1_000_000,
            data: bytes_of(&multisig_config).to_vec(),
            owner: program_id,
            ..Account::default()
        },
    );

    // === Create Record Account ===
    // data to be copied to another account
    let mut record_data = vec![0u8; 100];
    
    let record_key = Pubkey::new_unique();
    let record_header = RecordData {
        version: 1,
        authority: multisig_key,
    };
    
    let record_bytes = bytes_of(&record_header);
    record_data[..record_bytes.len()].copy_from_slice(record_bytes);

    program_test.add_account(
        record_key,
        Account {
            lamports: 1_000_000,
            data: record_data.clone(),
            owner: program_id,
            ..Account::default()
        },
    );

    // === Proposal Account ===
    // it needs to fit both the proposal metadata and the payload to transfer
    // NOTE the payload to transfer could also represent the instruction which needs
    // to be executed as part of the multisignature request 
    let proposal_key = Pubkey::new_unique();
    {
        let payload_space = record_data.len();
        let data = vec![0u8; Proposal::SIZE + payload_space]; // Proposal + payload space
        program_test.add_account(
            proposal_key,
            Account {
                lamports: 1_000_000,
                data,
                owner: program_id,
                ..Account::default()
            },
        );
    }

    // === Start Test Context ===
    let (banks_client, payer, recent_blockhash) = program_test.start().await;

    // === Instruction 1: Initialize Write Proposal ===
    let payload = b"hello!";
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true), // payer is signer & writable
            AccountMeta::new(proposal_key, false),  // writable (assume signer if needed)
            AccountMeta::new(record_key, false),    // writable
            AccountMeta::new_readonly(multisig_key, false), // readonly, not signer
        ],
        data: {
            let mut d = vec![5]; // instruction_tag = 5 (submit)
            d.extend_from_slice(&0u64.to_le_bytes()); // offset = 0
            d.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // data length to write
            d.extend_from_slice(payload); // data to write
            // instr + metadata + execute_data
            d
        },
    };
    
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    msg!("Processing first transaction!");
    banks_client.process_transaction(tx).await.unwrap();

    // === Instruction 2: Signer1 approves ===
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(signer1.pubkey(), true), // signer1: signer & writable
            AccountMeta::new(proposal_key, false),    // writable, not signer
            AccountMeta::new(record_key, false),      // writable, not signer
            AccountMeta::new_readonly(multisig_key, false), // readonly, not signer
        ],
        data: vec![6], // instruction_tag = 6 (approve)
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &signer1],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // === Instruction 3: Signer2 approves — triggers execution ===
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(signer2.pubkey(), true), // signer1: signer & writable
            AccountMeta::new(proposal_key, false),    // writable, not signer
            AccountMeta::new(record_key, false),      // writable, not signer
            AccountMeta::new_readonly(multisig_key, false), // readonly, not signer
        ],
        data: vec![6], // instruction_tag = 6 (approve)
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &signer2],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // === Verify Data Was Written ===
    let record_account = banks_client
        .get_account(record_key)
        .await
        .unwrap()
        .expect("record account should exist");

    let written = &record_account.data[33..33 + payload.len()];
    assert_eq!(written, payload);
}
