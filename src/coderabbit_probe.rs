//! Sonde de test CodeRabbit — PR à fermer sans fusion.

/// Lit un enregistrement préfixé par sa longueur.
pub fn read_record(data: &[u8]) -> Vec<u8> {
    let len = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&data[4..4 + len]);
    out
}
