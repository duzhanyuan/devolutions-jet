use lazy_static::lazy_static;

use crate::{
    encryption::{rc4, HASH_SIZE},
    ntlm::{
        messages::test::{get_test_identity, NTLM_VERSION},
        AuthenticateMessage, ChallengeMessage, Mic, NegotiateMessage, Ntlm, NtlmState, CHALLENGE_SIZE, SIGNATURE_SIZE,
    },
    sspi::*,
};

const TEST_SEQ_NUM: u32 = 1_234_567_890;
const SEALING_KEY: [u8; HASH_SIZE] = [
    0xa4, 0xf1, 0xba, 0xa6, 0x7c, 0xdc, 0x1a, 0x12, 0x20, 0xc0, 0x2b, 0x3d, 0xc0, 0x61, 0xa7, 0x73,
];
const SIGNING_KEY: [u8; HASH_SIZE] = [
    0x20, 0xc0, 0x2b, 0x3d, 0xc0, 0x61, 0xa7, 0x73, 0xa4, 0xf1, 0xba, 0xa6, 0x7c, 0xdc, 0x1a, 0x12,
];

lazy_static! {
    pub static ref TEST_DATA: Vec<u8> = b"Hello, World!!!".to_vec();
    pub static ref ENCRYPTED_TEST_DATA: Vec<u8> =
        vec![0x20, 0x2e, 0xdd, 0xd9, 0x56, 0x5e, 0xc4, 0x59, 0x42, 0xdb, 0x94, 0xfd, 0x6b, 0xf3, 0x11];
    pub static ref DIGEST_FOR_TEST_DATA: Vec<u8> = vec![0x58, 0x27, 0x4d, 0x35, 0x1f, 0x2d, 0x3c, 0xfd];
    pub static ref SIGNATURE_FOR_TEST_DATA: Vec<u8> =
        vec![0x1, 0x0, 0x0, 0x0, 0x58, 0x27, 0x4d, 0x35, 0x1f, 0x2d, 0x3c, 0xfd, 0xd2, 0x2, 0x96, 0x49];
}

#[test]
fn encrypt_message_crypts_data() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.send_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let input = &*TEST_DATA;
    let expected = &*ENCRYPTED_TEST_DATA;

    let output = context.encrypt_message(input, 0).unwrap();

    assert_eq!(expected.as_slice(), &output[SIGNATURE_SIZE..]);
}

#[test]
fn encrypt_message_correct_computes_digest() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.send_signing_key = SIGNING_KEY;
    context.send_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let input = &*TEST_DATA;
    let expected = &*DIGEST_FOR_TEST_DATA;

    let output = context.encrypt_message(input, TEST_SEQ_NUM).unwrap();

    assert_eq!(expected.as_slice(), &output[4..12]);
}

#[test]
fn encrypt_message_writes_seq_num_to_signature() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.send_signing_key = SIGNING_KEY;
    context.send_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let input = &*TEST_DATA;
    let expected = TEST_SEQ_NUM.to_le_bytes();

    let output = context.encrypt_message(input, TEST_SEQ_NUM).unwrap();

    assert_eq!(expected, output[12..SIGNATURE_SIZE]);
}

#[test]
fn decrypt_message_decrypts_data() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.recv_signing_key = SIGNING_KEY;
    context.recv_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let mut input = SIGNATURE_FOR_TEST_DATA.to_vec();
    input.extend_from_slice(&*ENCRYPTED_TEST_DATA);

    let expected = TEST_DATA.clone();

    let output = context.decrypt_message(&input, TEST_SEQ_NUM).unwrap();

    assert_eq!(expected, output);
}

#[test]
fn decrypt_message_does_not_fail_on_correct_signature() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.recv_signing_key = SIGNING_KEY;
    context.recv_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let mut input = SIGNATURE_FOR_TEST_DATA.to_vec();
    input.extend_from_slice(&*ENCRYPTED_TEST_DATA);

    let _output = context.decrypt_message(&input, TEST_SEQ_NUM).unwrap();
}

#[test]
fn decrypt_message_fails_on_incorrect_version() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.recv_signing_key = SIGNING_KEY;
    context.recv_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let mut input = vec![
        0x02, 0x00, 0x00, 0x00, 0x2e, 0xdf, 0xf2, 0x61, 0x29, 0xd6, 0x4d, 0xa9, 0xd2, 0x02, 0x96, 0x49,
    ];
    input.extend_from_slice(&*ENCRYPTED_TEST_DATA);

    assert!(context.decrypt_message(&input, TEST_SEQ_NUM).is_err());
}

#[test]
fn decrypt_message_fails_on_incorrect_checksum() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.recv_signing_key = SIGNING_KEY;
    context.recv_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let mut input = vec![
        0x01, 0x00, 0x00, 0x00, 0x2e, 0xdf, 0xff, 0x61, 0x29, 0xd6, 0x4d, 0xa9, 0xd2, 0x02, 0x96, 0x49,
    ];
    input.extend_from_slice(&*ENCRYPTED_TEST_DATA);

    assert!(context.decrypt_message(&input, TEST_SEQ_NUM).is_err());
}

#[test]
fn decrypt_message_fails_on_incorrect_seq_num() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.recv_signing_key = SIGNING_KEY;
    context.recv_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let mut input = vec![
        0x01, 0x00, 0x00, 0x00, 0x2e, 0xdf, 0xf2, 0x61, 0x29, 0xd6, 0x4d, 0xa9, 0xd2, 0x02, 0x96, 0x40,
    ];
    input.extend_from_slice(&*ENCRYPTED_TEST_DATA);

    assert!(context.decrypt_message(&input, TEST_SEQ_NUM).is_err());
}

#[test]
fn decrypt_message_fails_on_incorrect_signing_key() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.recv_signing_key = SEALING_KEY;
    context.recv_sealing_key = Some(rc4::Rc4::new(&SEALING_KEY));

    let mut input = SIGNATURE_FOR_TEST_DATA.to_vec();
    input.extend_from_slice(&*ENCRYPTED_TEST_DATA);

    assert!(context.decrypt_message(&input, TEST_SEQ_NUM).is_err());
}

#[test]
fn decrypt_message_fails_on_incorrect_sealing_key() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.recv_signing_key = SIGNING_KEY;
    context.recv_sealing_key = Some(rc4::Rc4::new(&SIGNING_KEY));

    let mut input = SIGNATURE_FOR_TEST_DATA.to_vec();
    input.extend_from_slice(&*ENCRYPTED_TEST_DATA);

    assert!(context.decrypt_message(&input, TEST_SEQ_NUM).is_err());
}

#[test]
fn initialize_security_context_wrong_state_negotiate() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Negotiate;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context
        .initialize_security_context(input.as_slice(), &mut output)
        .is_err());
    assert_eq!(context.state, NtlmState::Negotiate);
}

#[test]
fn initialize_security_context_wrong_state_authenticate() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Authenticate;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context
        .initialize_security_context(input.as_slice(), &mut output)
        .is_err());
    assert_eq!(context.state, NtlmState::Authenticate);
}

#[test]
fn initialize_security_context_wrong_state_completion() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Completion;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context
        .initialize_security_context(input.as_slice(), &mut output)
        .is_err());
    assert_eq!(context.state, NtlmState::Completion);
}

#[test]
fn initialize_security_context_wrong_state_final() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Final;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context
        .initialize_security_context(input.as_slice(), &mut output)
        .is_err());
    assert_eq!(context.state, NtlmState::Final);
}

#[test]
fn initialize_security_context_writes_negotiate_message() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Initial;

    let input = Vec::new();
    let mut output = Vec::new();

    assert_eq!(
        context.initialize_security_context(input.as_slice(), &mut output),
        Ok(SspiOk::ContinueNeeded)
    );
    assert_eq!(context.state, NtlmState::Challenge);
    assert!(!output.is_empty());
}

#[test]
fn initialize_security_context_reads_challenge_message() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Challenge;
    context.negotiate_message = Some(NegotiateMessage::new(Vec::new()));

    let input = vec![
        0x4e, 0x54, 0x4c, 0x4d, 0x53, 0x53, 0x50, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x30, 0x00,
        0x00, 0x00, 0x97, 0x82, 0x88, 0xe0, 0xfe, 0x14, 0x51, 0x74, 0x06, 0x57, 0x92, 0x8a, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x31, 0x00, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];
    let mut output = Vec::new();

    assert_eq!(
        context.initialize_security_context(input.as_slice(), &mut output),
        Ok(SspiOk::CompleteNeeded)
    );
    assert_ne!(context.state, NtlmState::Challenge);
}

#[test]
fn initialize_security_context_writes_authenticate_message() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Challenge;
    context.negotiate_message = Some(NegotiateMessage::new(Vec::new()));

    let input = vec![
        0x4e, 0x54, 0x4c, 0x4d, 0x53, 0x53, 0x50, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x30, 0x00,
        0x00, 0x00, 0x97, 0x82, 0x88, 0xe0, 0xfe, 0x14, 0x51, 0x74, 0x06, 0x57, 0x92, 0x8a, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x31, 0x00, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];
    let mut output = Vec::new();

    assert_eq!(
        context.initialize_security_context(input.as_slice(), &mut output),
        Ok(SspiOk::CompleteNeeded)
    );
    assert_eq!(context.state, NtlmState::Final);
    assert!(!output.is_empty());
}

#[test]
fn initialize_security_context_fails_on_empty_output_on_challenge_state() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Challenge;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context
        .initialize_security_context(input.as_slice(), &mut output)
        .is_err());
}

#[test]
fn accept_security_context_wrong_state_negotiate() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Negotiate;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context.accept_security_context(input.as_slice(), &mut output).is_err());
    assert_eq!(context.state, NtlmState::Negotiate);
}

#[test]
fn accept_security_context_wrong_state_challenge() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Challenge;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context.accept_security_context(input.as_slice(), &mut output).is_err());
    assert_eq!(context.state, NtlmState::Challenge);
}

#[test]
fn accept_security_context_wrong_state_completion() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Completion;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context.accept_security_context(input.as_slice(), &mut output).is_err());
    assert_eq!(context.state, NtlmState::Completion);
}

#[test]
fn accept_security_context_wrong_state_final() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Final;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context.accept_security_context(input.as_slice(), &mut output).is_err());
    assert_eq!(context.state, NtlmState::Final);
}

#[test]
fn accept_security_context_reads_negotiate_message() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Initial;

    let input = vec![
        0x4e, 0x54, 0x4c, 0x4d, 0x53, 0x53, 0x50, 0x00, 0x01, 0x00, 0x00, 0x00, 0x97, 0x82, 0x08, 0xe0, 0x00, 0x00,
        0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
    ];
    let mut output = Vec::new();

    assert_eq!(
        context.accept_security_context(input.as_slice(), &mut output),
        Ok(SspiOk::ContinueNeeded)
    );
    assert_ne!(context.state, NtlmState::Challenge);
}

#[test]
fn accept_security_context_writes_challenge_message() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Initial;

    let input = vec![
        0x4e, 0x54, 0x4c, 0x4d, 0x53, 0x53, 0x50, 0x00, 0x01, 0x00, 0x00, 0x00, 0x97, 0x82, 0x08, 0xe0, 0x00, 0x00,
        0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
    ];
    let mut output = Vec::new();

    assert_eq!(
        context.accept_security_context(input.as_slice(), &mut output),
        Ok(SspiOk::ContinueNeeded)
    );
    assert_eq!(context.state, NtlmState::Authenticate);
    assert!(!output.is_empty());
}

#[test]
fn accept_security_context_reads_authenticate() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Authenticate;
    context.negotiate_message = Some(NegotiateMessage::new(vec![0x01, 0x02, 0x03]));
    context.challenge_message = Some(ChallengeMessage::new(
        vec![0x04, 0x05, 0x06],
        Vec::new(),
        [0x00; CHALLENGE_SIZE],
        0,
    ));

    let input = vec![
        0x4e, 0x54, 0x4c, 0x4d, 0x53, 0x53, 0x50, 0x00, // signature
        0x03, 0x00, 0x00, 0x00, // message type
        0x18, 0x00, 0x18, 0x00, 0x55, 0x00, 0x00, 0x00, // LmChallengeResponseFields
        0x30, 0x00, 0x30, 0x00, 0x6d, 0x00, 0x00, 0x00, // NtChallengeResponseFields
        0x06, 0x00, 0x06, 0x00, 0x40, 0x00, 0x00, 0x00, // DomainNameFields
        0x04, 0x00, 0x04, 0x00, 0x46, 0x00, 0x00, 0x00, // UserNameFields
        0x0b, 0x00, 0x0b, 0x00, 0x4a, 0x00, 0x00, 0x00, // WorkstationFields
        0x10, 0x00, 0x10, 0x00, 0x9d, 0x00, 0x00, 0x00, // EncryptedRandomSessionKeyFields
        0x35, 0xb2, 0x08, 0xe0, // NegotiateFlags
        0x44, 0x6f, 0x6d, 0x61, 0x69, 0x6e, // domain
        0x55, 0x73, 0x65, 0x72, // user
        0x57, 0x6f, 0x72, 0x6b, 0x73, 0x74, 0x61, 0x74, 0x69, 0x6f, 0x6e, // workstation
        0x13, 0x23, 0x04, 0xd8, 0x5f, 0x66, 0x52, 0xce, 0x41, 0xd6, 0xa9, 0x98, 0xf6, 0xbc, 0x73, 0x1b, 0x04, 0xd8,
        0x5f, 0x41, 0xd6, 0xa9, 0x5f, 0x66, // lm challenge
        0x1f, 0x7b, 0x1d, 0x2a, 0x15, 0xf5, 0x5d, 0x95, 0xc3, 0xce, 0x90, 0xbd, 0x10, 0x1e, 0xe3, 0xa8, 0x01, 0x01,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x33, 0x57, 0xbd, 0xb1, 0x07, 0x8b, 0xcf, 0x01, 0x20, 0xc0, 0x2b, 0x3d,
        0xc0, 0x61, 0xa7, 0x73, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nt challenge
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, // encrypted key
    ];
    let mut output = Vec::new();

    assert_eq!(
        context.accept_security_context(input.as_slice(), &mut output),
        Ok(SspiOk::CompleteNeeded)
    );
    assert_eq!(context.state, NtlmState::Completion);
}

#[test]
fn accept_security_context_fails_on_empty_output_on_negotiate_state() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Initial;

    let input = Vec::new();
    let mut output = Vec::new();

    assert!(context.accept_security_context(input.as_slice(), &mut output).is_err());
}

#[test]
fn complete_auth_token_fails_on_incorrect_state() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Authenticate;

    assert!(context.complete_auth_token().is_err());
}

#[test]
fn complete_auth_token_changes_state() {
    let mut context = Ntlm::new(get_test_identity(), NTLM_VERSION);
    context.state = NtlmState::Completion;
    context.negotiate_message = Some(NegotiateMessage::new(vec![0x01, 0x02, 0x03]));
    context.challenge_message = Some(ChallengeMessage::new(
        vec![0x04, 0x05, 0x06],
        Vec::new(),
        [0x00; CHALLENGE_SIZE],
        0,
    ));
    context.authenticate_message = Some(AuthenticateMessage::new(
        vec![
            0x4e, 0x54, 0x4c, 0x4d, 0x53, 0x53, 0x50, 0x00, 0x03, 0x00, 0x00, 0x00, 0x18, 0x00, 0x18, 0x00, 0x65, 0x00,
            0x00, 0x00, 0x68, 0x00, 0x68, 0x00, 0x7d, 0x00, 0x00, 0x00, 0x06, 0x00, 0x06, 0x00, 0x50, 0x00, 0x00, 0x00,
            0x04, 0x00, 0x04, 0x00, 0x56, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x0b, 0x00, 0x5a, 0x00, 0x00, 0x00, 0x10, 0x00,
            0x10, 0x00, 0xe5, 0x00, 0x00, 0x00, 0x35, 0xb2, 0x88, 0xe0, 0x10, 0xb9, 0x77, 0x7d, 0x0, 0xd, 0xfd, 0x89,
            0xe4, 0x1c, 0xfb, 0x92, 0x40, 0x2f, 0x4a, 0x3e, 0x44, 0x6f, 0x6d, 0x61, 0x69, 0x6e, 0x55, 0x73, 0x65, 0x72,
            0x57, 0x6f, 0x72, 0x6b, 0x73, 0x74, 0x61, 0x74, 0x69, 0x6f, 0x6e, 0x13, 0x23, 0x04, 0xd8, 0x5f, 0x66, 0x52,
            0xce, 0x41, 0xd6, 0xa9, 0x98, 0xf6, 0xbc, 0x73, 0x1b, 0x04, 0xd8, 0x5f, 0x41, 0xd6, 0xa9, 0x5f, 0x66, 0xdc,
            0xe1, 0xf1, 0x59, 0xc4, 0xcd, 0x25, 0xc9, 0xd3, 0xe5, 0xfa, 0x3a, 0xf4, 0xb9, 0x81, 0xa9, 0x01, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x33, 0x57, 0xbd, 0xb1, 0x07, 0x8b, 0xcf, 0x01, 0x00, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x04, 0x00, 0x56, 0x47, 0x50, 0x43, 0x01, 0x00, 0x04,
            0x00, 0x56, 0x47, 0x50, 0x43, 0x04, 0x00, 0x04, 0x00, 0x56, 0x47, 0x50, 0x43, 0x03, 0x00, 0x04, 0x00, 0x56,
            0x47, 0x50, 0x43, 0x07, 0x00, 0x08, 0x00, 0x33, 0x57, 0xbd, 0xb1, 0x07, 0x8b, 0xcf, 0x01, 0x06, 0x00, 0x04,
            0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        ],
        Some(Mic::new(
            [
                0x9, 0x28, 0xb9, 0x1, 0xf9, 0xa6, 0x74, 0x85, 0x99, 0xb5, 0x31, 0xad, 0xd6, 0x5d, 0x4f, 0xa3,
            ],
            64,
        )),
        [
            0x35, 0x33, 0x2a, 0x3b, 0xc2, 0xb6, 0x35, 0xbb, 0xda, 0x6a, 0xfb, 0xc6, 0xff, 0x50, 0xf3, 0x0f,
        ],
    ));

    context.complete_auth_token().unwrap();
    assert_eq!(context.state, NtlmState::Final);
}