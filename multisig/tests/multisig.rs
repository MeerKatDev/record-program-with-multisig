use {
    multisig::config::MultisigConfig,
    all2all_controller::{
        processor::process_instruction,
        state::RecordData,
    },
    bytemuck::bytes_of,
    solana_pubkey::Pubkey,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_test::*,
    solana_sdk::{
        account::Account,
        signature::{Keypair, Signer},
        transaction::Transaction,
    },
};

#[tokio::test]
async fn test_multisig_write_approval_execution() {
    // === Setup Program Test Environment ===
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new("all2all_controller", program_id, processor!(process_instruction));

    // === Create Multisig Config ===
    let signer1 = Keypair::new();
    let signer2 = Keypair::new();
    let signer3 = Keypair::new();
    let signers = [signer1.pubkey(), signer2.pubkey(), signer3.pubkey()];

    let multisig_key = Pubkey::new_unique();
    let multisig_config = MultisigConfig {
        version: MultisigConfig::CURRENT_VERSION,
        threshold: 2,
        signer_count: 3,
        signers: {
            let mut padded = [Pubkey::default(); 10];
            padded[..3].copy_from_slice(&signers);
            padded
        },
    };
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
    let record_key = Pubkey::new_unique();
    let mut record_data = vec![0u8; 100];
    let record_header = RecordData {
        version: 1,
        authority: multisig_key,
    };
    record_data[..bytes_of(&record_header).len()].copy_from_slice(bytes_of(&record_header));
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
    let proposal_key = Pubkey::new_unique();
    let proposal_data = vec![0u8; 64 + 8]; // Proposal + payload space
    program_test.add_account(
        proposal_key,
        Account {
            lamports: 1_000_000,
            data: proposal_data,
            owner: program_id,
            ..Account::default()
        },
    );

    // === Start Test Context ===
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // === Instruction 1: Initialize Write Proposal ===
    let payload = b"hello!";
    let ix = Instruction {
        program_id,
        accounts: vec![
		    AccountMeta::new(payer.pubkey(), true),            // payer is signer & writable
		    AccountMeta::new(proposal_key, false),              // writable (assume signer if needed)
		    AccountMeta::new(record_key, false),                // writable
		    AccountMeta::new_readonly(multisig_key, false),    // readonly, not signer
        ],
        data: {
            let mut d = vec![5]; // instruction_tag = 5 (submit)
            d.extend_from_slice(&0u64.to_le_bytes()); // offset = 0
            d.extend_from_slice(payload); // data to write
            d
        },
    };
    // not enough signers
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // === Instruction 2: Signer1 approves ===
    let ix = Instruction {
        program_id,
        accounts: vec![
		    AccountMeta::new(signer1.pubkey(), true),       // signer1: signer & writable
		    AccountMeta::new(proposal_key, false),          // writable, not signer
		    AccountMeta::new(record_key, false),            // writable, not signer
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
		    AccountMeta::new(signer2.pubkey(), true),       // signer1: signer & writable
		    AccountMeta::new(proposal_key, false),          // writable, not signer
		    AccountMeta::new(record_key, false),            // writable, not signer
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
