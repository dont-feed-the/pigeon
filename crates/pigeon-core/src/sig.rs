//! Commit signatures (ADR 0007, 04 §4.4): an SSHSIG, namespace `git`, in
//! the `gpgsig` header of a commit object, as `git commit -S` writes it with
//! `gpg.format=ssh`, and its check.
//!
//! The signed bytes are the commit object without its `gpgsig` header, as
//! git's `parse_buffer_signed_by_header` (`commit.c`) cuts them; the
//! signer is the ed25519 key the signature carries, as a device ID: the
//! hex of that key. Who may sign what is [`crate::authority`]'s question.
//!
//! Laws (`tests/sig.rs`): a signed commit verifies to its signer; changing
//! any byte of a signed commit never verifies to a signer; signing and
//! cutting the signature off again gives the unsigned commit back.

use ssh_key::{HashAlg, LineEnding, PrivateKey, PublicKey, SshSig};

use crate::roster::DeviceId;

/// The SSHSIG namespace of git's commit signatures.
pub const NAMESPACE: &str = "git";
const HEADER: &[u8] = b"gpgsig ";

/// Why a commit has no valid signer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SigError {
    /// It has no `gpgsig` header.
    Unsigned,
    /// It has more than one.
    TwoSignatures,
    /// The header is not an SSH signature pigeon reads.
    Unreadable,
    /// The signature is for another namespace than `git`.
    Namespace(String),
    /// The key is not ed25519.
    NotEd25519,
    /// The signature does not match the commit.
    Mismatch,
    /// The commit already has a signature (when signing).
    AlreadySigned,
    /// Signing failed.
    Signing,
}

/// The device ID of an ed25519 public key: its 64 hex digits.
pub fn device_id_of(public: &[u8; 32]) -> DeviceId {
    DeviceId(public.iter().map(|b| format!("{b:02x}")).collect())
}

/// The device ID of a device's secret key.
pub fn device_id(secret: &[u8; 32]) -> DeviceId {
    let key = keypair(secret);
    device_id_of(&key.public.0)
}

fn keypair(secret: &[u8; 32]) -> ssh_key::private::Ed25519Keypair {
    ssh_key::private::Ed25519Keypair::from(ssh_key::private::Ed25519PrivateKey::from_bytes(secret))
}

/// The end of a commit's header: the index of the empty line's newline,
/// or the length if there is no message.
fn header_end(raw: &[u8]) -> usize {
    raw.windows(2)
        .position(|w| w == b"\n\n")
        .map_or(raw.len(), |i| i + 1)
}

/// Splits a commit object into its signed bytes and its signatures, each
/// as the text of its `gpgsig` header without the continuation spaces.
pub fn split(raw: &[u8]) -> (Vec<u8>, Vec<String>) {
    let end = header_end(raw);
    let (head, body) = raw.split_at(end);
    let mut payload = Vec::with_capacity(raw.len());
    let mut sigs: Vec<String> = Vec::new();
    let mut in_sig = false;
    for line in head.split_inclusive(|b| *b == b'\n') {
        if let Some(rest) = line.strip_prefix(HEADER) {
            sigs.push(String::from_utf8_lossy(rest).into_owned());
            in_sig = true;
        } else if in_sig && line.first() == Some(&b' ') {
            if let (Some(s), Some(rest)) = (sigs.last_mut(), line.get(1..)) {
                s.push_str(&String::from_utf8_lossy(rest));
            }
        } else {
            in_sig = false;
            payload.extend_from_slice(line);
        }
    }
    payload.extend_from_slice(body);
    (payload, sigs)
}

/// The signer of a commit object, if its one signature is valid.
pub fn verify(raw: &[u8]) -> Result<DeviceId, SigError> {
    let (payload, sigs) = split(raw);
    let pem = match sigs.as_slice() {
        [] => return Err(SigError::Unsigned),
        [one] => one,
        _ => return Err(SigError::TwoSignatures),
    };
    let sig = SshSig::from_pem(pem.as_bytes()).map_err(|_| SigError::Unreadable)?;
    if sig.namespace() != NAMESPACE {
        return Err(SigError::Namespace(sig.namespace().to_owned()));
    }
    let Some(key) = sig.public_key().ed25519() else {
        return Err(SigError::NotEd25519);
    };
    let id = device_id_of(&key.0);
    PublicKey::from(sig.public_key().clone())
        .verify(NAMESPACE, &payload, &sig)
        .map_err(|_| SigError::Mismatch)?;
    Ok(id)
}

/// Signs an unsigned commit object with a device's secret key, as git
/// would: the `gpgsig` header goes at the end of the header.
pub fn sign(raw: &[u8], secret: &[u8; 32]) -> Result<Vec<u8>, SigError> {
    if !split(raw).1.is_empty() {
        return Err(SigError::AlreadySigned);
    }
    let key = PrivateKey::from(keypair(secret));
    let pem = SshSig::sign(&key, NAMESPACE, HashAlg::Sha512, raw)
        .and_then(|s| s.to_pem(LineEnding::LF))
        .map_err(|_| SigError::Signing)?;
    let end = header_end(raw);
    let (head, body) = raw.split_at(end);
    let mut out = Vec::with_capacity(raw.len() + pem.len() + 16);
    out.extend_from_slice(head);
    for (i, line) in pem.split_inclusive('\n').enumerate() {
        out.extend_from_slice(if i == 0 { HEADER } else { b" " });
        out.extend_from_slice(line.as_bytes());
    }
    if !pem.ends_with('\n') {
        out.push(b'\n');
    }
    out.extend_from_slice(body);
    Ok(out)
}

/// The secret key as an OpenSSH private key file, for `ssh-keygen -Y sign`
/// and `git commit -S` (ADR 0007).
pub fn openssh_private(secret: &[u8; 32]) -> Option<String> {
    PrivateKey::from(keypair(secret))
        .to_openssh(LineEnding::LF)
        .ok()
        .map(|s| s.to_string())
}

/// The public key as an OpenSSH `ssh-ed25519 …` line, for an allowed
/// signers file.
pub fn openssh_public(secret: &[u8; 32]) -> Option<String> {
    PrivateKey::from(keypair(secret))
        .public_key()
        .to_openssh()
        .ok()
}
