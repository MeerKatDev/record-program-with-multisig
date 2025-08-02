// use spl_record::{
//     multisig::{config::MultisigConfig, processor::*},
//     state::RecordData,
//     Proposal,
//     MAX_SIGNERS,
// };
// use solana_program_test::ProgramTest;
// use solana_program_test::processor;
// use solana_program::{pubkey::Pubkey, instruction::Instruction};
// use bytemuck::{Pod, Zeroable};
// use all2all_controller::multisig::instructions::*;
// use all2all_controller::processor::process_instruction;

// fn program_test() -> ProgramTest {
//     ProgramTest::new("all2all_controller", id(), processor!(process_instruction))
// }

// #[tokio::test]
// async fn test_multisig_write_executes_on_threshold() {
//     let mut ctx = program_test().start_with_context().await;

//     let mut payer = ctx.payer().clone();

//     // Setup: multisig with 3 signers, threshold = 2
//     let signer_keys: Vec<Pubkey> = (0..3).map(|_| ctx.gen_key()).collect();
//     let signer_infos = signer_keys
//         .iter()
//         .map(|k| ctx.with_signer(*k))
//         .collect::<Vec<_>>();

//     let multisig_data = MultisigConfig {
//         version: MultisigConfig::CURRENT_VERSION,
//         threshold: 2,
//         signer_count: 3,
//         signers: {
//             let mut arr = [Pubkey::default(); MAX_SIGNERS];
//             arr[..3].copy_from_slice(&signer_keys[..3]);
//             arr
//         },
//     };

//     let multisig_account = ctx.create_account(bytemuck::bytes_of(&multisig_data), false).await;

//     // Record account setup
//     let mut record_bytes = vec![0u8; 100]; // Make sure this fits the written offset
//     let record_authority = *multisig_account.key();
//     record_bytes[..33].copy_from_slice(bytemuck::bytes_of(&RecordData {
//         version: 1,
//         authority: record_authority,
//     }));
//     let record_account = ctx.create_account(&record_bytes, true).await;

//     // Proposal account with room for metadata + payload
//     let payload = b"hello multisig";
//     let proposal_space = Proposal::DATA_START_INDEX + payload.len();
//     let proposal_account = ctx.create_zeroed_account(proposal_space, true).await;

//     // Submit proposal
//     process_multisig_write(
//         &[
//             proposal_account.clone(),
//             record_account.clone(),
//             multisig_account.clone(),
//             payer.clone(),
//         ],
//         0, // offset
//         payload,
//     )
//     .unwrap();

//     // First approval by signer 0
//     process_approve_proposal(&[
//         signer_infos[0].clone(),
//         proposal_account.clone(),
//         record_account.clone(),
//         multisig_account.clone(),
//     ])
//     .unwrap();

//     // At this point, write has NOT occurred yet
//     let record_data = record_account.try_borrow_data().unwrap();
//     assert_ne!(
//         &record_data[RecordData::WRITABLE_START_INDEX..][..payload.len()],
//         payload
//     );

//     // Second approval by signer 1 - this should trigger execution
//     process_approve_proposal(&[
//         signer_infos[1].clone(),
//         proposal_account.clone(),
//         record_account.clone(),
//         multisig_account.clone(),
//     ])
//     .unwrap();

//     // Verify write occurred
//     let record_data = record_account.try_borrow_data().unwrap();
//     assert_eq!(
//         &record_data[RecordData::WRITABLE_START_INDEX..][..payload.len()],
//         payload
//     );

//     // Confirm proposal marked as executed
//     let proposal_data = proposal_account.try_borrow_data().unwrap();
//     let (meta, _) = proposal_data.split_at(Proposal::DATA_START_INDEX);
//     let proposal: Proposal = *bytemuck::from_bytes(&meta[..std::mem::size_of::<Proposal>()]);
//     assert!(proposal.is_executed());
// }
