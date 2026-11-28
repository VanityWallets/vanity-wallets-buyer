use bech32::{hrp, segwit, Bech32, Hrp};
use secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::OnceLock;

const REQ_TAG: &str = "VGREQ1";
const REQ_TAG_V2: &str = "VGREQ2";
const PROOF_TAG_V1: &str = "VGPROOF1";
const KEY_TAG_V1: &str = "VGKEY1";
const PROOF_TAG: &str = "VGPROOF2";
const KEY_TAG: &str = "VGKEY2";
const PROOF_TAG_V3: &str = "VGPROOF3";
const KEY_TAG_V3: &str = "VGKEY3";
const PROOF_TAG_V4: &str = "VGPROOF4";
const KEY_TAG_V4: &str = "VGKEY4";
pub const MAX_HALVES: usize = 100;
const POINT_HEX: usize = 66;
pub const MULTIPLIERS: u8 = 6;
const LAMBDA: [u8; 32] = [
    0x53, 0x63, 0xad, 0x4c, 0xc0, 0x5c, 0x30, 0xe0, 0xa5, 0x26, 0x1c, 0x02, 0x88, 0x12, 0x64, 0x5a,
    0x12, 0x2e, 0x22, 0xea, 0x20, 0x81, 0x66, 0x78, 0xdf, 0x02, 0x96, 0x7c, 0x1b, 0x23, 0xbd, 0x72,
];

fn lambda() -> Scalar {
    Scalar::from_be_bytes(LAMBDA).expect("lambda is below the group order")
}

pub fn scale_point(p: &PublicKey, mult: u8) -> Result<PublicKey, SplitError> {
    if mult >= MULTIPLIERS {
        return Err(SplitError::BadFormat);
    }
    let mut q = *p;
    for _ in 0..mult / 2 {
        q = q.mul_tweak(secp(), &lambda()).map_err(|_| SplitError::BadKey)?;
    }
    Ok(if mult % 2 == 1 { q.negate(secp()) } else { q })
}

pub fn scale_secret(k: &SecretKey, mult: u8) -> Result<SecretKey, SplitError> {
    if mult >= MULTIPLIERS {
        return Err(SplitError::BadFormat);
    }
    let mut s = *k;
    for _ in 0..mult / 2 {
        s = s.mul_tweak(&lambda()).map_err(|_| SplitError::BadKey)?;
    }
    Ok(if mult % 2 == 1 { s.negate() } else { s })
}

fn parse_mult(s: &str) -> Result<u8, SplitError> {
    match s {
        "1" | "2" | "3" | "4" | "5" => Ok(s.as_bytes()[0] - b'0'),
        _ => Err(SplitError::BadFormat),
    }
}

fn secp() -> &'static Secp256k1<secp256k1::All> {
    static CTX: OnceLock<Secp256k1<secp256k1::All>> = OnceLock::new();
    CTX.get_or_init(Secp256k1::new)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Tron,
    Eth,
    BtcP2pkh,
    BtcP2sh,
    BtcP2wpkh,
    BtcP2tr,
    Bsc,
    Polygon,
    Arbitrum,
    Optimism,
    Base,
    Avalanche,
    Linea,
    ZkSync,
    HyperEvm,
    LtcP2pkh,
    LtcP2sh,
    LtcP2wpkh,
    Doge,
    Dash,
    Xrp,
    Cosmos,
    Osmosis,
    Celestia,
    Dydx,
    Akash,
    Juno,
    Stargaze,
    Noble,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Tron,
    Evm,
    Base58 { version: u8, p2sh: bool, ripple: bool },
    SegwitV0 { hrp: &'static str },
    Taproot,
    Bech32 { hrp: &'static str },
}

#[allow(dead_code)]
pub const RIPPLE_ALPHABET: &[u8; 58] = b"rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz";

impl Network {
    pub const ALL: [Network; 29] = [
        Network::Tron,
        Network::Eth,
        Network::Bsc,
        Network::Polygon,
        Network::Arbitrum,
        Network::Optimism,
        Network::Base,
        Network::Avalanche,
        Network::Linea,
        Network::ZkSync,
        Network::HyperEvm,
        Network::BtcP2pkh,
        Network::BtcP2sh,
        Network::BtcP2wpkh,
        Network::BtcP2tr,
        Network::LtcP2pkh,
        Network::LtcP2sh,
        Network::LtcP2wpkh,
        Network::Doge,
        Network::Dash,
        Network::Xrp,
        Network::Cosmos,
        Network::Osmosis,
        Network::Celestia,
        Network::Dydx,
        Network::Akash,
        Network::Juno,
        Network::Stargaze,
        Network::Noble,
    ];

    pub fn tag(self) -> &'static str {
        match self {
            Network::Tron => "TRON",
            Network::Eth => "ETH",
            Network::BtcP2pkh => "BTC-P2PKH",
            Network::BtcP2sh => "BTC-P2SH",
            Network::BtcP2wpkh => "BTC-P2WPKH",
            Network::BtcP2tr => "BTC-P2TR",
            Network::Bsc => "BSC",
            Network::Polygon => "POLYGON",
            Network::Arbitrum => "ARBITRUM",
            Network::Optimism => "OPTIMISM",
            Network::Base => "BASE",
            Network::Avalanche => "AVAX-C",
            Network::Linea => "LINEA",
            Network::ZkSync => "ZKSYNC",
            Network::HyperEvm => "HYPEREVM",
            Network::LtcP2pkh => "LTC-P2PKH",
            Network::LtcP2sh => "LTC-P2SH",
            Network::LtcP2wpkh => "LTC-P2WPKH",
            Network::Doge => "DOGE",
            Network::Dash => "DASH",
            Network::Xrp => "XRP",
            Network::Cosmos => "COSMOS",
            Network::Osmosis => "OSMOSIS",
            Network::Celestia => "CELESTIA",
            Network::Dydx => "DYDX",
            Network::Akash => "AKASH",
            Network::Juno => "JUNO",
            Network::Stargaze => "STARGAZE",
            Network::Noble => "NOBLE",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Network> {
        Network::ALL.into_iter().find(|n| n.tag() == tag)
    }

    pub fn label(self) -> &'static str {
        match self {
            Network::Tron => "TRON (TRX / TRC20)",
            Network::Eth => "Ethereum (ETH / ERC20)",
            Network::BtcP2pkh => "Bitcoin Legacy (1…)",
            Network::BtcP2sh => "Bitcoin P2SH-SegWit (3…)",
            Network::BtcP2wpkh => "Bitcoin Native SegWit (bc1q…)",
            Network::BtcP2tr => "Bitcoin Taproot (bc1p…)",
            Network::Bsc => "BNB Smart Chain (BSC / BEP20)",
            Network::Polygon => "Polygon (POL)",
            Network::Arbitrum => "Arbitrum One",
            Network::Optimism => "Optimism (OP Mainnet)",
            Network::Base => "Base",
            Network::Avalanche => "Avalanche C-Chain",
            Network::Linea => "Linea",
            Network::ZkSync => "zkSync Era",
            Network::HyperEvm => "HyperEVM (HYPE)",
            Network::LtcP2pkh => "Litecoin Legacy (L…)",
            Network::LtcP2sh => "Litecoin P2SH-SegWit (M…)",
            Network::LtcP2wpkh => "Litecoin Native SegWit (ltc1q…)",
            Network::Doge => "Dogecoin (D…)",
            Network::Dash => "Dash (X…)",
            Network::Xrp => "XRP Ledger (r…)",
            Network::Cosmos => "Cosmos Hub (cosmos1…)",
            Network::Osmosis => "Osmosis (osmo1…)",
            Network::Celestia => "Celestia (celestia1…)",
            Network::Dydx => "dYdX (dydx1…)",
            Network::Akash => "Akash (akash1…)",
            Network::Juno => "Juno (juno1…)",
            Network::Stargaze => "Stargaze (stars1…)",
            Network::Noble => "Noble (noble1…)",
        }
    }

    pub fn encoding(self) -> Encoding {
        let b58 = |version, p2sh| Encoding::Base58 { version, p2sh, ripple: false };
        match self {
            Network::Tron => Encoding::Tron,
            Network::Eth
            | Network::Bsc
            | Network::Polygon
            | Network::Arbitrum
            | Network::Optimism
            | Network::Base
            | Network::Avalanche
            | Network::Linea
            | Network::ZkSync
            | Network::HyperEvm => Encoding::Evm,
            Network::BtcP2pkh => b58(0x00, false),
            Network::BtcP2sh => b58(0x05, true),
            Network::BtcP2wpkh => Encoding::SegwitV0 { hrp: "bc" },
            Network::BtcP2tr => Encoding::Taproot,
            Network::LtcP2pkh => b58(0x30, false),
            Network::LtcP2sh => b58(0x32, true),
            Network::LtcP2wpkh => Encoding::SegwitV0 { hrp: "ltc" },
            Network::Doge => b58(0x1e, false),
            Network::Dash => b58(0x4c, false),
            Network::Xrp => Encoding::Base58 { version: 0x00, p2sh: false, ripple: true },
            Network::Cosmos => Encoding::Bech32 { hrp: "cosmos" },
            Network::Osmosis => Encoding::Bech32 { hrp: "osmo" },
            Network::Celestia => Encoding::Bech32 { hrp: "celestia" },
            Network::Dydx => Encoding::Bech32 { hrp: "dydx" },
            Network::Akash => Encoding::Bech32 { hrp: "akash" },
            Network::Juno => Encoding::Bech32 { hrp: "juno" },
            Network::Stargaze => Encoding::Bech32 { hrp: "stars" },
            Network::Noble => Encoding::Bech32 { hrp: "noble" },
        }
    }

    pub fn is_evm(self) -> bool {
        self.encoding() == Encoding::Evm
    }

    pub fn wif_version(self) -> Option<u8> {
        match self {
            Network::BtcP2pkh | Network::BtcP2sh | Network::BtcP2wpkh | Network::BtcP2tr => Some(0x80),
            Network::LtcP2pkh | Network::LtcP2sh | Network::LtcP2wpkh => Some(0xb0),
            Network::Doge => Some(0x9e),
            Network::Dash => Some(0xcc),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitError {
    BadFormat,
    BadChecksum,
    BadKey,
    UnknownNetwork,
    WrongOrder,
    AddressMismatch,
    ProofMismatch,
    WrongIndex,
    BadSignature,
}

impl SplitError {
    pub fn text(self, ru: bool) -> &'static str {
        match (self, ru) {
            (SplitError::BadFormat, true) => "Это не код нужного вида. Скопируйте его целиком.",
            (SplitError::BadFormat, false) => "This is not a code of the expected kind. Copy it in full.",
            (SplitError::BadChecksum, true) => "Код повреждён (не сошлась контрольная сумма). Скопируйте его заново.",
            (SplitError::BadChecksum, false) => "The code is damaged (checksum mismatch). Copy it again.",
            (SplitError::BadKey, true) => "В коде недопустимый ключ.",
            (SplitError::BadKey, false) => "The code contains an invalid key.",
            (SplitError::UnknownNetwork, true) => "В коде неизвестная сеть.",
            (SplitError::UnknownNetwork, false) => "The code names an unknown network.",
            (SplitError::WrongOrder, true) => "Код относится к другой заявке.",
            (SplitError::WrongOrder, false) => "The code belongs to another request.",
            (SplitError::AddressMismatch, true) => {
                "Адрес НЕ получается из вашей половины ключа. Не используйте этот адрес!"
            }
            (SplitError::AddressMismatch, false) => {
                "The address does NOT come from your key half. Do not use this address!"
            }
            (SplitError::ProofMismatch, true) => {
                "Присланная половина не совпадает с той, что вы проверяли раньше."
            }
            (SplitError::ProofMismatch, false) => {
                "The received half does not match the one you verified earlier."
            }
            (SplitError::WrongIndex, true) => "Номер адреса в коде не подходит к этой заявке.",
            (SplitError::WrongIndex, false) => "The address number in the code does not fit this request.",
            (SplitError::BadSignature, true) => {
                "Продавец не подтвердил, что знает свою половину ключа (подпись не сошлась). Не оплачивайте!"
            }
            (SplitError::BadSignature, false) => {
                "The seller did not prove knowing their key half (the signature does not match). Do not pay!"
            }
        }
    }
}

impl fmt::Display for SplitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.text(false))
    }
}

fn hash160(data: &[u8]) -> [u8; 20] {
    use ripemd::Ripemd160;
    let mut out = [0u8; 20];
    out.copy_from_slice(&Ripemd160::digest(Sha256::digest(data)));
    out
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    use tiny_keccak::{Hasher, Keccak};
    let mut out = [0u8; 32];
    let mut k = Keccak::v256();
    k.update(data);
    k.finalize(&mut out);
    out
}

fn base58check(version: u8, body: &[u8]) -> String {
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(version);
    payload.extend_from_slice(body);
    bs58::encode(payload).with_check().into_string()
}

fn eip55(addr20: &[u8]) -> String {
    let lower: String = addr20.iter().map(|b| format!("{b:02x}")).collect();
    let hash = keccak256(lower.as_bytes());
    let mut out = String::from("0x");
    for (i, c) in lower.chars().enumerate() {
        let nibble = if i % 2 == 0 { hash[i / 2] >> 4 } else { hash[i / 2] & 0xf };
        out.push(if c.is_ascii_alphabetic() && nibble >= 8 { c.to_ascii_uppercase() } else { c });
    }
    out
}

fn taproot_output(pk: &PublicKey) -> Option<[u8; 32]> {
    let (internal, _) = pk.x_only_public_key();
    let tag = Sha256::digest(b"TapTweak");
    let mut h = Sha256::new();
    h.update(tag);
    h.update(tag);
    h.update(internal.serialize());
    let tweak = Scalar::from_be_bytes(h.finalize().into()).ok()?;
    let (output, _) = internal.add_tweak(secp(), &tweak).ok()?;
    Some(output.serialize())
}

pub fn address(net: Network, pk: &PublicKey) -> String {
    match net.encoding() {
        Encoding::Tron => {
            let hash = keccak256(&pk.serialize_uncompressed()[1..]);
            base58check(0x41, &hash[12..])
        }
        Encoding::Evm => eip55(&keccak256(&pk.serialize_uncompressed()[1..])[12..]),
        Encoding::Base58 { version, p2sh, ripple } => {
            let mut h = hash160(&pk.serialize());
            if p2sh {
                let mut redeem = vec![0x00, 0x14];
                redeem.extend_from_slice(&h);
                h = hash160(&redeem);
            }
            if ripple {
                let mut payload = vec![version];
                payload.extend_from_slice(&h);
                bs58::encode(payload).with_alphabet(bs58::Alphabet::RIPPLE).with_check().into_string()
            } else {
                base58check(version, &h)
            }
        }
        Encoding::SegwitV0 { hrp } => {
            let hrp = Hrp::parse(hrp).expect("valid hrp");
            segwit::encode_v0(hrp, &hash160(&pk.serialize())).expect("20-byte witness program")
        }
        Encoding::Taproot => match taproot_output(pk) {
            Some(x) => segwit::encode_v1(hrp::BC, &x).expect("32-byte witness program"),
            None => String::new(),
        },
        Encoding::Bech32 { hrp } => {
            let hrp = Hrp::parse(hrp).expect("valid hrp");
            bech32::encode::<Bech32>(hrp, &hash160(&pk.serialize())).expect("20-byte program")
        }
    }
}

pub fn export_secret(net: Network, k: &SecretKey) -> String {
    match net.wif_version() {
        Some(version) => {
            let mut payload = Vec::with_capacity(33);
            payload.extend_from_slice(&k.secret_bytes());
            payload.push(0x01);
            base58check(version, &payload)
        }
        None => hex(&k.secret_bytes()),
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

fn with_checksum(body: String) -> String {
    let sum = Sha256::digest(body.as_bytes());
    format!("{body}:{}", hex(&sum[..4]))
}

fn parse_fields(code: &str, tag: &str, n_fields: usize) -> Result<Vec<String>, SplitError> {
    let clean: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let (body, sum) = clean.rsplit_once(':').ok_or(SplitError::BadFormat)?;
    let fields: Vec<&str> = body.split(':').collect();
    if fields.len() != n_fields + 1 || !fields[0].eq_ignore_ascii_case(tag) {
        return Err(SplitError::BadFormat);
    }
    let expected = Sha256::digest(format!("{tag}:{}", fields[1..].join(":")).as_bytes());
    if !sum.eq_ignore_ascii_case(&hex(&expected[..4])) {
        return Err(SplitError::BadChecksum);
    }
    Ok(fields[1..].iter().map(|s| s.to_string()).collect())
}

pub fn find_codes(text: &str, kind: &str) -> Vec<String> {
    let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let upper = clean.to_ascii_uppercase();
    let kind = kind.to_ascii_uppercase();
    let mut starts: Vec<(usize, bool)> = Vec::new();
    for t in [REQ_TAG, REQ_TAG_V2, PROOF_TAG_V1, PROOF_TAG, PROOF_TAG_V3, PROOF_TAG_V4, KEY_TAG_V1, KEY_TAG, KEY_TAG_V3, KEY_TAG_V4] {
        let wanted = t.starts_with(&kind);
        starts.extend(upper.match_indices(&format!("{t}:")).map(|(i, _)| (i, wanted)));
    }
    starts.sort_unstable();
    starts
        .iter()
        .enumerate()
        .filter(|(_, (_, wanted))| *wanted)
        .map(|(n, &(s, _))| {
            let piece = &clean[s..starts.get(n + 1).map_or(clean.len(), |&(next, _)| next)];
            let end = piece
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == ':' || c == '-'))
                .unwrap_or(piece.len());
            let code = &piece[..end];
            match code.rfind(':') {
                Some(i) if code.len() > i + 9 => code[..i + 9].to_string(),
                _ => code.to_string(),
            }
        })
        .collect()
}

fn parse_public(s: &str) -> Result<PublicKey, SplitError> {
    let bytes = unhex(&s.to_ascii_lowercase()).ok_or(SplitError::BadKey)?;
    if bytes.len() != 33 {
        return Err(SplitError::BadKey);
    }
    PublicKey::from_slice(&bytes).map_err(|_| SplitError::BadKey)
}

pub fn order_id(a_pub: &PublicKey) -> String {
    hex(&Sha256::digest(a_pub.serialize())[..4]).to_ascii_uppercase()
}

fn parse_index(s: &str) -> Result<u32, SplitError> {
    if s.is_empty() || s.len() > 3 || s.starts_with('0') || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SplitError::BadFormat);
    }
    match s.parse::<u32>() {
        Ok(i) if i >= 1 && i as usize <= MAX_HALVES => Ok(i),
        _ => Err(SplitError::BadFormat),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub points: Vec<PublicKey>,
    pub legacy: bool,
}

impl Request {
    pub fn single(a_pub: PublicKey) -> Request {
        Request { points: vec![a_pub], legacy: true }
    }

    pub fn many(points: Vec<PublicKey>) -> Request {
        Request { points, legacy: false }
    }

    pub fn encode(&self) -> String {
        if self.legacy {
            return with_checksum(format!("{REQ_TAG}:{}", hex(&self.points[0].serialize())));
        }
        let body: String = self.points.iter().map(|p| hex(&p.serialize())).collect();
        with_checksum(format!("{REQ_TAG_V2}:{}:{body}", self.points.len()))
    }

    pub fn decode(code: &str) -> Result<Request, SplitError> {
        if !is_tag(code, REQ_TAG_V2) {
            let f = parse_fields(code, REQ_TAG, 1)?;
            return Ok(Request::single(parse_public(&f[0])?));
        }
        let f = parse_fields(code, REQ_TAG_V2, 2)?;
        let n = parse_index(&f[0])? as usize;
        if f[1].len() != n * POINT_HEX || !f[1].is_ascii() {
            return Err(SplitError::BadFormat);
        }
        let mut points: Vec<PublicKey> = Vec::with_capacity(n);
        for i in 0..n {
            let p = parse_public(&f[1][i * POINT_HEX..(i + 1) * POINT_HEX])?;
            if points.contains(&p) {
                return Err(SplitError::BadKey);
            }
            points.push(p);
        }
        Ok(Request::many(points))
    }

    pub fn order_id(&self) -> String {
        if self.legacy {
            return order_id(&self.points[0]);
        }
        let mut h = Sha256::new();
        h.update(REQ_TAG_V2.as_bytes());
        for p in &self.points {
            h.update(p.serialize());
        }
        hex(&h.finalize()[..4]).to_ascii_uppercase()
    }

    pub fn count(&self) -> usize {
        self.points.len()
    }

    pub fn point(&self, index: Option<u32>) -> Result<&PublicKey, SplitError> {
        match (self.legacy, index) {
            (true, None) => Ok(&self.points[0]),
            (false, Some(i)) if i >= 1 && i as usize <= self.points.len() => Ok(&self.points[i as usize - 1]),
            _ => Err(SplitError::WrongIndex),
        }
    }
}

fn proof_digest(order_id: &str, index: u32, net: Network, mult: u8, a_pub: &PublicKey, b_pub: &PublicKey) -> secp256k1::Message {
    let mut h = Sha256::new();
    if mult == 0 {
        h.update(format!("{PROOF_TAG_V3}|{order_id}|{index}|{}|", net.tag()).as_bytes());
    } else {
        h.update(format!("{PROOF_TAG_V4}|{order_id}|{index}|{}|{mult}|", net.tag()).as_bytes());
    }
    h.update(hex(&a_pub.serialize()).as_bytes());
    h.update(b"|");
    h.update(hex(&b_pub.serialize()).as_bytes());
    secp256k1::Message::from_digest(h.finalize().into())
}

#[derive(Clone, Debug, Eq)]
pub struct Proof {
    pub order_id: String,
    pub index: Option<u32>,
    pub net: Network,
    pub claimed: Option<String>,
    pub b_pub: PublicKey,
    pub signature: Option<[u8; 64]>,
    pub mult: u8,
}

impl PartialEq for Proof {
    fn eq(&self, other: &Self) -> bool {
        self.order_id == other.order_id
            && self.index == other.index
            && self.net == other.net
            && self.b_pub == other.b_pub
            && self.mult == other.mult
    }
}

impl Proof {
    pub fn encode(&self) -> String {
        match (self.index, self.signature) {
            (Some(i), Some(sig)) if self.mult != 0 => with_checksum(format!(
                "{PROOF_TAG_V4}:{}:{i}:{}:{}:{}:{}",
                self.order_id,
                self.net.tag(),
                self.mult,
                hex(&self.b_pub.serialize()),
                hex(&sig)
            )),
            (Some(i), Some(sig)) => with_checksum(format!(
                "{PROOF_TAG_V3}:{}:{i}:{}:{}:{}",
                self.order_id,
                self.net.tag(),
                hex(&self.b_pub.serialize()),
                hex(&sig)
            )),
            _ => with_checksum(format!(
                "{PROOF_TAG}:{}:{}:{}",
                self.order_id,
                self.net.tag(),
                hex(&self.b_pub.serialize())
            )),
        }
    }

    pub fn decode(code: &str) -> Result<Proof, SplitError> {
        let signature_of = |s: &str| -> Result<[u8; 64], SplitError> {
            let sig_bytes = unhex(&s.to_ascii_lowercase()).ok_or(SplitError::BadSignature)?;
            sig_bytes.try_into().map_err(|_| SplitError::BadSignature)
        };
        if is_tag(code, PROOF_TAG_V4) {
            let f = parse_fields(code, PROOF_TAG_V4, 6)?;
            return Ok(Proof {
                order_id: f[0].to_ascii_uppercase(),
                index: Some(parse_index(&f[1])?),
                net: Network::from_tag(&f[2]).ok_or(SplitError::UnknownNetwork)?,
                claimed: None,
                mult: parse_mult(&f[3])?,
                b_pub: parse_public(&f[4])?,
                signature: Some(signature_of(&f[5])?),
            });
        }
        if is_tag(code, PROOF_TAG_V3) {
            let f = parse_fields(code, PROOF_TAG_V3, 5)?;
            return Ok(Proof {
                order_id: f[0].to_ascii_uppercase(),
                index: Some(parse_index(&f[1])?),
                net: Network::from_tag(&f[2]).ok_or(SplitError::UnknownNetwork)?,
                claimed: None,
                b_pub: parse_public(&f[3])?,
                signature: Some(signature_of(&f[4])?),
                mult: 0,
            });
        }
        if let Ok(f) = parse_fields(code, PROOF_TAG, 3) {
            return Ok(Proof {
                order_id: f[0].to_ascii_uppercase(),
                index: None,
                net: Network::from_tag(&f[1]).ok_or(SplitError::UnknownNetwork)?,
                claimed: None,
                b_pub: parse_public(&f[2])?,
                signature: None,
                mult: 0,
            });
        }
        let v1 = parse_fields(code, PROOF_TAG_V1, 4);
        let f = match v1 {
            Ok(f) => f,
            Err(e) if is_tag(code, PROOF_TAG_V1) => return Err(e),
            Err(_) => return Err(parse_fields(code, PROOF_TAG, 3).err().unwrap_or(SplitError::BadFormat)),
        };
        Ok(Proof {
            order_id: f[0].to_ascii_uppercase(),
            index: None,
            net: Network::from_tag(&f[1]).ok_or(SplitError::UnknownNetwork)?,
            claimed: Some(f[2].clone()),
            b_pub: parse_public(&f[3])?,
            signature: None,
            mult: 0,
        })
    }

    #[allow(dead_code)]
    pub fn signed(request: &Request, index: u32, net: Network, b: &SecretKey) -> Result<Proof, SplitError> {
        Proof::signed_with(request, index, net, b, 0)
    }

    #[allow(dead_code)]
    pub fn signed_with(request: &Request, index: u32, net: Network, b: &SecretKey, mult: u8) -> Result<Proof, SplitError> {
        if mult >= MULTIPLIERS {
            return Err(SplitError::BadFormat);
        }
        let a_pub = request.point(Some(index))?;
        let b_pub = PublicKey::from_secret_key(secp(), b);
        let order_id = request.order_id();
        let msg = proof_digest(&order_id, index, net, mult, a_pub, &b_pub);
        let signature = secp().sign_ecdsa(&msg, b).serialize_compact();
        Ok(Proof { order_id, index: Some(index), net, claimed: None, b_pub, signature: Some(signature), mult })
    }

    pub fn verify(&self, request: &Request) -> Result<String, SplitError> {
        if self.order_id != request.order_id() {
            return Err(SplitError::WrongOrder);
        }
        let a_pub = request.point(self.index)?;
        if self.index.is_none() && self.mult != 0 {
            return Err(SplitError::BadFormat);
        }
        if let Some(index) = self.index {
            let sig = self
                .signature
                .and_then(|s| secp256k1::ecdsa::Signature::from_compact(&s).ok())
                .ok_or(SplitError::BadSignature)?;
            let msg = proof_digest(&self.order_id, index, self.net, self.mult, a_pub, &self.b_pub);
            secp().verify_ecdsa(&msg, &sig, &self.b_pub).map_err(|_| SplitError::BadSignature)?;
        }
        let q = scale_point(a_pub, self.mult)?.combine(&self.b_pub).map_err(|_| SplitError::BadKey)?;
        let derived = address(self.net, &q);
        if derived.is_empty() || self.claimed.as_ref().is_some_and(|c| *c != derived) {
            return Err(SplitError::AddressMismatch);
        }
        Ok(derived)
    }
}

fn is_tag(code: &str, tag: &str) -> bool {
    let clean: String = code.chars().filter(|c| !c.is_whitespace()).take(tag.len() + 1).collect();
    clean.eq_ignore_ascii_case(&format!("{tag}:"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyHalf {
    pub order_id: String,
    pub index: Option<u32>,
    pub net: Network,
    pub claimed: Option<String>,
    pub b: SecretKey,
    pub mult: u8,
}

impl KeyHalf {
    #[allow(dead_code)]
    pub fn encode(&self) -> String {
        match self.index {
            Some(i) if self.mult != 0 => with_checksum(format!(
                "{KEY_TAG_V4}:{}:{i}:{}:{}:{}",
                self.order_id,
                self.net.tag(),
                self.mult,
                hex(&self.b.secret_bytes())
            )),
            Some(i) => with_checksum(format!(
                "{KEY_TAG_V3}:{}:{i}:{}:{}",
                self.order_id,
                self.net.tag(),
                hex(&self.b.secret_bytes())
            )),
            None => with_checksum(format!("{KEY_TAG}:{}:{}:{}", self.order_id, self.net.tag(), hex(&self.b.secret_bytes()))),
        }
    }

    pub fn decode(code: &str) -> Result<KeyHalf, SplitError> {
        let (f, index, claimed, mult) = if is_tag(code, KEY_TAG_V4) {
            let f = parse_fields(code, KEY_TAG_V4, 5)?;
            let index = parse_index(&f[1])?;
            (vec![f[0].clone(), f[2].clone(), f[4].clone()], Some(index), None, parse_mult(&f[3])?)
        } else if is_tag(code, KEY_TAG_V3) {
            let f = parse_fields(code, KEY_TAG_V3, 4)?;
            let index = parse_index(&f[1])?;
            (vec![f[0].clone(), f[2].clone(), f[3].clone()], Some(index), None, 0)
        } else {
            match parse_fields(code, KEY_TAG, 3) {
                Ok(f) => (f, None, None, 0),
                Err(e2) => match parse_fields(code, KEY_TAG_V1, 4) {
                    Ok(f) => (vec![f[0].clone(), f[1].clone(), f[3].clone()], None, Some(f[2].clone()), 0),
                    Err(e1) if is_tag(code, KEY_TAG_V1) => return Err(e1),
                    Err(_) => return Err(e2),
                },
            }
        };
        let mut bytes = unhex(&f[2].to_ascii_lowercase()).ok_or(SplitError::BadKey)?;
        let b = SecretKey::from_slice(&bytes).map_err(|_| SplitError::BadKey);
        for x in bytes.iter_mut() {
            unsafe { std::ptr::write_volatile(x, 0) };
        }
        Ok(KeyHalf {
            order_id: f[0].to_ascii_uppercase(),
            index,
            net: Network::from_tag(&f[1]).ok_or(SplitError::UnknownNetwork)?,
            claimed,
            b: b?,
            mult,
        })
    }

    pub fn proof(&self) -> Proof {
        Proof {
            order_id: self.order_id.clone(),
            index: self.index,
            net: self.net,
            claimed: self.claimed.clone(),
            b_pub: PublicKey::from_secret_key(secp(), &self.b),
            signature: None,
            mult: self.mult,
        }
    }

    pub fn verify(&self, request: &Request) -> Result<String, SplitError> {
        if self.order_id != request.order_id() {
            return Err(SplitError::WrongOrder);
        }
        let a_pub = request.point(self.index)?;
        if self.index.is_none() && self.mult != 0 {
            return Err(SplitError::BadFormat);
        }
        let q = scale_point(a_pub, self.mult)?.combine(&self.proof().b_pub).map_err(|_| SplitError::BadKey)?;
        let derived = address(self.net, &q);
        if derived.is_empty() || self.claimed.as_ref().is_some_and(|c| *c != derived) {
            return Err(SplitError::AddressMismatch);
        }
        Ok(derived)
    }

    pub fn combine(&self, a: &SecretKey, request: &Request, verified: Option<&Proof>) -> Result<(SecretKey, String), SplitError> {
        if PublicKey::from_secret_key(secp(), a) != *request.point(self.index)? {
            return Err(SplitError::WrongIndex);
        }
        if let Some(p) = verified {
            if *p != self.proof() {
                return Err(SplitError::ProofMismatch);
            }
        }
        let expected = self.verify(request)?;
        let mut scaled = scale_secret(a, self.mult)?;
        let k = scaled
            .add_tweak(&Scalar::from_be_bytes(self.b.secret_bytes()).map_err(|_| SplitError::BadKey)?)
            .map_err(|_| SplitError::BadKey);
        scaled.non_secure_erase();
        let k = k?;
        let from_key = address(self.net, &PublicKey::from_secret_key(secp(), &k));
        if from_key != expected {
            return Err(SplitError::AddressMismatch);
        }
        Ok((k, from_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> SecretKey {
        let mut b = [0u8; 32];
        b[31] = n;
        SecretKey::from_slice(&b).unwrap()
    }

    #[test]
    fn known_addresses_for_key_one() {
        let g = PublicKey::from_secret_key(secp(), &key(1));
        assert_eq!(address(Network::Eth, &g), "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf");
        assert_eq!(address(Network::BtcP2pkh, &g), "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
        assert_eq!(address(Network::BtcP2sh, &g), "3JvL6Ymt8MVWiCNHC7oWU6nLeHNJKLZGLN");
        assert_eq!(address(Network::BtcP2wpkh, &g), "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
        assert_eq!(
            address(Network::BtcP2tr, &g),
            "bc1pmfr3p9j00pfxjh0zmgp99y8zftmd3s5pmedqhyptwy6lm87hf5sspknck9"
        );
        assert_eq!(
            export_secret(Network::BtcP2pkh, &key(1)),
            "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn"
        );
        assert!(address(Network::Tron, &g).starts_with('T'));
    }

    #[test]
    fn new_networks_for_key_one() {
        let g = PublicKey::from_secret_key(secp(), &key(1));
        let h = unhex("751e76e8199196d454941c45d1b3a323f1433bd6").unwrap();
        let eth = address(Network::Eth, &g);
        for net in Network::ALL.into_iter().filter(|n| n.is_evm()) {
            assert_eq!(address(net, &g), eth, "{net:?}");
            assert_eq!(export_secret(net, &key(1)), hex(&key(1).secret_bytes()));
        }
        for (net, version, first) in [
            (Network::LtcP2pkh, 0x30, 'L'),
            (Network::Doge, 0x1e, 'D'),
            (Network::Dash, 0x4c, 'X'),
        ] {
            let a = address(net, &g);
            let raw = bs58::decode(&a).with_check(None).into_vec().unwrap();
            assert_eq!((raw[0], &raw[1..]), (version, &h[..]), "{a}");
            assert!(a.starts_with(first) && a.len() == 34, "{a}");
        }
        let m = address(Network::LtcP2sh, &g);
        assert!(m.starts_with('M') && bs58::decode(&m).with_check(Some(0x32)).into_vec().is_ok(), "{m}");
        assert_eq!(&address(Network::LtcP2wpkh, &g)[..37], "ltc1qw508d6qejxtdg4y5r3zarvary0c5xw7k");
        for (net, hrp) in [(Network::Cosmos, "cosmos"), (Network::Osmosis, "osmo"), (Network::Noble, "noble")] {
            let a = address(net, &g);
            let (got_hrp, data) = bech32::decode(&a).unwrap();
            assert_eq!((got_hrp.as_str(), &data[..]), (hrp, &h[..]), "{a}");
            assert!(a.starts_with(&format!("{hrp}1w508d6qejxtdg4y5r3zarvary0c5xw7k")), "{a}");
        }
        for (net, version) in [(Network::LtcP2pkh, 0xb0), (Network::Doge, 0x9e), (Network::Dash, 0xcc)] {
            let raw = bs58::decode(export_secret(net, &key(1))).with_check(Some(version)).into_vec().unwrap();
            assert_eq!(&raw[1..], &[&key(1).secret_bytes()[..], &[1]].concat()[..]);
        }
    }

    #[test]
    fn xrp_genesis_address() {
        let pk = PublicKey::from_slice(
            &unhex("0330e7fc9d56bb25d6893ba3f317ae5bcf33b3291bd63db32654a313222f7fd020").unwrap(),
        )
        .unwrap();
        assert_eq!(address(Network::Xrp, &pk), "rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh");
        assert_eq!(export_secret(Network::Xrp, &key(1)), hex(&key(1).secret_bytes()));
    }

    #[test]
    fn tags_are_unique_and_stable() {
        let tags: std::collections::HashSet<_> = Network::ALL.iter().map(|n| n.tag()).collect();
        let labels: std::collections::HashSet<_> = Network::ALL.iter().map(|n| n.label()).collect();
        assert_eq!((tags.len(), labels.len()), (Network::ALL.len(), Network::ALL.len()));
        let old: Vec<_> = [
            Network::Tron,
            Network::Eth,
            Network::BtcP2pkh,
            Network::BtcP2sh,
            Network::BtcP2wpkh,
            Network::BtcP2tr,
        ]
        .iter()
        .map(|n| n.tag())
        .collect();
        assert_eq!(old, ["TRON", "ETH", "BTC-P2PKH", "BTC-P2SH", "BTC-P2WPKH", "BTC-P2TR"]);
    }

    fn half_for(request: &Request, net: Network, b: SecretKey) -> (KeyHalf, String) {
        let q = request.points[0].combine(&PublicKey::from_secret_key(secp(), &b)).unwrap();
        let addr = address(net, &q);
        (KeyHalf { order_id: request.order_id(), index: None, net, claimed: Some(addr.clone()), b, mult: 0 }, addr)
    }

    fn legacy(a: &SecretKey) -> Request {
        Request::single(PublicKey::from_secret_key(secp(), a))
    }

    #[test]
    fn full_cycle_every_network() {
        for net in Network::ALL {
            let a = key(7);
            let request = Request::decode(&legacy(&a).encode()).unwrap();
            let (half, addr) = half_for(&request, net, key(200));
            let half = KeyHalf { claimed: None, ..half };
            let proof_code = half.proof().encode();
            let key_code = half.encode();
            assert!(proof_code.starts_with("VGPROOF2:") && key_code.starts_with("VGKEY2:"));
            assert!(!proof_code.contains(&addr) && !key_code.contains(&addr), "{net:?}");
            let proof = Proof::decode(&proof_code).unwrap();
            assert_eq!(proof.claimed, None);
            assert_eq!(proof.verify(&request).unwrap(), addr);
            let half = KeyHalf::decode(&key_code).unwrap();
            let (k, from_key) = half.combine(&a, &request, Some(&proof)).unwrap();
            assert_eq!(k, key(207));
            assert_eq!(from_key, addr);
        }
    }

    #[test]
    fn version_one_codes_still_work() {
        let a = key(11);
        let request = legacy(&a);
        let (half, addr) = half_for(&request, Network::BtcP2pkh, key(33));
        let b_pub = hex(&PublicKey::from_secret_key(secp(), &half.b).serialize());
        let v1_proof = with_checksum(format!("VGPROOF1:{}:BTC-P2PKH:{addr}:{b_pub}", request.order_id()));
        let v1_key = with_checksum(format!("VGKEY1:{}:BTC-P2PKH:{addr}:{}", request.order_id(), hex(&half.b.secret_bytes())));
        let proof = Proof::decode(&v1_proof).unwrap();
        assert_eq!(proof.claimed.as_deref(), Some(addr.as_str()));
        assert_eq!(proof.verify(&request).unwrap(), addr);
        assert_eq!(proof, Proof::decode(&half.proof().encode()).unwrap());
        let (_, from_key) = KeyHalf::decode(&v1_key).unwrap().combine(&a, &request, Some(&proof)).unwrap();
        assert_eq!(from_key, addr);
        let fake = address(Network::BtcP2pkh, &PublicKey::from_secret_key(secp(), &key(5)));
        let forged = with_checksum(format!("VGPROOF1:{}:BTC-P2PKH:{fake}:{b_pub}", request.order_id()));
        assert_eq!(Proof::decode(&forged).unwrap().verify(&request), Err(SplitError::AddressMismatch));
        let damaged = format!("{}0", &v1_proof[..v1_proof.len() - 1]);
        let damaged = if damaged == v1_proof { format!("{}1", &v1_proof[..v1_proof.len() - 1]) } else { damaged };
        assert_eq!(Proof::decode(&damaged), Err(SplitError::BadChecksum));
    }

    #[test]
    fn damaged_or_foreign_codes_are_rejected() {
        let a = key(3);
        let request = legacy(&a);
        let code = request.encode();
        let wrapped = format!("  {}\n{} ", &code[..20], &code[20..]);
        assert_eq!(Request::decode(&wrapped).unwrap(), request);
        let mut damaged = code.clone().into_bytes();
        damaged[10] = if damaged[10] == b'a' { b'b' } else { b'a' };
        assert!(Request::decode(std::str::from_utf8(&damaged).unwrap()).is_err());

        let (half, _) = half_for(&request, Network::Tron, key(9));
        let mut forged = half.proof();
        forged.claimed = Some(address(Network::Tron, &PublicKey::from_secret_key(secp(), &key(5))));
        assert_eq!(forged.verify(&request), Err(SplitError::AddressMismatch));
        let other = legacy(&key(4));
        assert_eq!(half.proof().verify(&other), Err(SplitError::WrongOrder));
        let mut other_proof = half.proof();
        other_proof.b_pub = PublicKey::from_secret_key(secp(), &key(10));
        assert_eq!(half.combine(&a, &request, Some(&other_proof)), Err(SplitError::ProofMismatch));
        assert!(half.combine(&key(4), &request, None).is_err());
        assert!(KeyHalf::decode(&half.proof().encode()).is_err());
        assert!(Proof::decode(&half.encode()).is_err());
    }

    #[test]
    fn many_codes_are_found() {
        let request = legacy(&key(3));
        let mut proofs: Vec<String> = [Network::BtcP2pkh, Network::Tron, Network::Avalanche]
            .into_iter()
            .enumerate()
            .map(|(i, net)| half_for(&request, net, key(20 + i as u8)).0.proof().encode())
            .collect();
        let (old, addr) = half_for(&request, Network::Avalanche, key(22));
        let b_pub = hex(&PublicKey::from_secret_key(secp(), &old.b).serialize());
        proofs[2] = with_checksum(format!("VGPROOF1:{}:AVAX-C:{addr}:{b_pub}", request.order_id()));
        let lines = proofs.join("\n");
        let glued = proofs.concat();
        let json = format!("\"Codes\": [\n    \"{}\",\n    \"{}\",\n    \"{}\"\n  ],", proofs[0], proofs[1], proofs[2]);
        let wrapped = format!("{}\n{}\r\n{}", &lines[..30], &lines[30..], "VGKEY2:zzz");
        let tail = format!("{}Hello {} then text\n{}", proofs[0], proofs[1], proofs[2]);
        for text in [lines, glued, json, wrapped, tail] {
            let found = find_codes(&text, "VGPROOF");
            assert_eq!(found, proofs, "{text}");
            assert!(found.iter().all(|c| Proof::decode(c).is_ok()));
        }
        assert!(find_codes("nothing here", "VGPROOF").is_empty());
    }

    fn many_request(n: u8) -> (Vec<SecretKey>, Request) {
        let secrets: Vec<SecretKey> = (0..n).map(|i| key(100 + i)).collect();
        let points = secrets.iter().map(|a| PublicKey::from_secret_key(secp(), a)).collect();
        (secrets, Request::many(points))
    }

    #[test]
    fn every_address_has_its_own_half() {
        let (secrets, request) = many_request(3);
        let code = request.encode();
        assert!(code.starts_with("VGREQ2:3:"));
        let back = Request::decode(&format!(" {}\n{} ", &code[..40], &code[40..])).unwrap();
        assert_eq!(back, request);
        assert_eq!(back.order_id(), request.order_id());
        assert_ne!(request.order_id(), order_id(&request.points[0]));
        let found = find_codes(&format!("Код заявки: {code}\nтекст"), "VGREQ");
        assert_eq!(found, std::slice::from_ref(&code));

        let mut keys: Vec<SecretKey> = Vec::new();
        for (i, a) in secrets.iter().enumerate() {
            let index = i as u32 + 1;
            let b = key(50 + i as u8);
            let proof = Proof::signed(&request, index, Network::Tron, &b).unwrap();
            let proof_code = proof.encode();
            assert!(proof_code.starts_with(&format!("VGPROOF3:{}:{index}:TRON:", request.order_id())));
            let proof = Proof::decode(&proof_code).unwrap();
            let addr = proof.verify(&request).unwrap();
            let expected = address(Network::Tron, &PublicKey::from_secret_key(secp(), &a.add_tweak(&Scalar::from(b)).unwrap()));
            assert_eq!(addr, expected);
            let half = KeyHalf { order_id: request.order_id(), index: Some(index), net: Network::Tron, claimed: None, b, mult: 0 };
            let key_code = half.encode();
            assert!(key_code.starts_with("VGKEY3:"));
            let half = KeyHalf::decode(&key_code).unwrap();
            let (k, from_key) = half.combine(a, &request, Some(&proof)).unwrap();
            assert_eq!(from_key, addr);
            for other in &secrets {
                if other != a {
                    assert_eq!(half.combine(other, &request, Some(&proof)), Err(SplitError::WrongIndex));
                }
            }
            keys.push(k);
        }
        let diff = |x: &SecretKey, y: &SecretKey| x.add_tweak(&Scalar::from(y.negate())).ok();
        assert_ne!(diff(&keys[0], &secrets[0]), diff(&keys[1], &secrets[0]));
    }

    #[test]
    fn unsigned_or_misplaced_proofs_are_rejected() {
        let (_, request) = many_request(2);
        let b = key(77);
        let good = Proof::signed(&request, 1, Network::Eth, &b).unwrap();
        assert!(good.verify(&request).is_ok());

        let unsigned = Proof { index: None, signature: None, ..good.clone() };
        assert_eq!(unsigned.verify(&request), Err(SplitError::WrongIndex));
        let v2 = Proof::decode(&unsigned.encode()).unwrap();
        assert_eq!(v2.verify(&request), Err(SplitError::WrongIndex));

        let moved = Proof { index: Some(2), ..good.clone() };
        assert_eq!(moved.verify(&request), Err(SplitError::BadSignature));
        let other_b = Proof { b_pub: PublicKey::from_secret_key(secp(), &key(78)), ..good.clone() };
        assert_eq!(other_b.verify(&request), Err(SplitError::BadSignature));
        let mut sig = good.signature.unwrap();
        sig[5] ^= 1;
        assert_eq!(Proof { signature: Some(sig), ..good.clone() }.verify(&request), Err(SplitError::BadSignature));
        assert_eq!(Proof { index: Some(3), ..good.clone() }.verify(&request), Err(SplitError::WrongIndex));

        let x = key(90);
        let rogue_pub = PublicKey::from_secret_key(secp(), &x).combine(&request.points[0].negate(secp())).unwrap();
        let rogue = Proof { b_pub: rogue_pub, ..good.clone() };
        assert_eq!(rogue.verify(&request), Err(SplitError::BadSignature));

        let legacy_request = legacy(&key(5));
        assert_eq!(good.verify(&legacy_request), Err(SplitError::WrongOrder));
        let legacy_signed = Proof { order_id: legacy_request.order_id(), ..good.clone() };
        assert_eq!(legacy_signed.verify(&legacy_request), Err(SplitError::WrongIndex));
        assert!(Proof::signed(&legacy_request, 1, Network::Eth, &b).is_err());
    }

    #[test]
    #[ignore = "prints codes for the site test"]
    fn print_interop_codes() {
        let (secrets, request) = many_request(4);
        println!("REQ {}", request.encode());
        for net in [Network::Tron, Network::Eth, Network::BtcP2tr, Network::Cosmos] {
            for (i, a) in secrets.iter().enumerate() {
                let b = key(60 + i as u8);
                let index = i as u32 + 1;
                let proof = Proof::signed(&request, index, net, &b).unwrap();
                let addr = proof.verify(&request).unwrap();
                let half = KeyHalf { order_id: request.order_id(), index: Some(index), net, claimed: None, b, mult: 0 };
                let (_, from_key) = half.combine(a, &request, Some(&proof)).unwrap();
                assert_eq!(from_key, addr);
                println!("ROW {} {} {} {} {}", net.tag(), index, proof.encode(), half.encode(), addr);
            }
        }
    }

    #[test]
    fn bad_many_requests_are_rejected() {
        let (_, request) = many_request(2);
        let code = request.encode();
        let body = code.rsplit_once(':').unwrap().0;
        let fields: Vec<&str> = body.split(':').collect();
        let points = fields[2];
        for bad_body in [
            format!("VGREQ2:3:{points}"),
            "VGREQ2:0:".to_string(),
            format!("VGREQ2:101:{points}"),
            format!("VGREQ2:02:{points}"),
            format!("VGREQ2:2:{}{}", &points[..66], &points[..66]),
            format!("VGREQ2:2:{}", &points[..131]),
        ] {
            let with = with_checksum(bad_body.clone());
            assert!(Request::decode(&with).is_err(), "{bad_body}");
        }
        assert_eq!(Request::decode(&format!("{body}:00000000")), Err(SplitError::BadChecksum));
    }

    #[test]
    fn multipliers_are_the_curve_symmetries() {
        let one = key(1);
        let mut cube = one;
        for _ in 0..3 {
            cube = scale_secret(&cube, 2).unwrap();
        }
        assert_eq!(cube, one);
        assert_ne!(scale_secret(&one, 2).unwrap(), one);
        let g = PublicKey::from_secret_key(secp(), &one);
        for m in 0..MULTIPLIERS {
            let s = scale_secret(&key(9), m).unwrap();
            assert_eq!(PublicKey::from_secret_key(secp(), &s), scale_point(&PublicKey::from_secret_key(secp(), &key(9)), m).unwrap());
            let p = scale_point(&g, m).unwrap().serialize_uncompressed();
            let x_same = p[1..33] == g.serialize_uncompressed()[1..33];
            assert_eq!(x_same, m < 2, "{m}");
        }
        assert!(scale_point(&g, MULTIPLIERS).is_err() && scale_secret(&one, MULTIPLIERS).is_err());
    }

    #[test]
    fn every_multiplier_gives_a_working_wallet() {
        let secrets: Vec<SecretKey> = (0..3).map(|i| key(60 + i)).collect();
        let request = Request::many(secrets.iter().map(|a| PublicKey::from_secret_key(secp(), a)).collect());
        for net in [Network::Tron, Network::Eth, Network::BtcP2pkh, Network::BtcP2wpkh, Network::LtcP2sh, Network::Xrp, Network::Cosmos] {
            for m in 0..MULTIPLIERS {
                let index = u32::from(m % 3) + 1;
                let b = key(100 + m);
                let a = &secrets[index as usize - 1];
                let proof = Proof::signed_with(&request, index, net, &b, m).unwrap();
                let code = proof.encode();
                assert!(code.starts_with(if m == 0 { "VGPROOF3:" } else { "VGPROOF4:" }), "{code}");
                let back = Proof::decode(&code).unwrap();
                assert_eq!((back.mult, back.index), (m, Some(index)));
                let address = back.verify(&request).unwrap();
                let expected_point = scale_point(&request.points[index as usize - 1], m).unwrap().combine(&PublicKey::from_secret_key(secp(), &b)).unwrap();
                assert_eq!(address, super::address(net, &expected_point));

                let half = KeyHalf { order_id: request.order_id(), index: Some(index), net, claimed: None, b, mult: m };
                let key_code = half.encode();
                assert!(key_code.starts_with(if m == 0 { "VGKEY3:" } else { "VGKEY4:" }), "{key_code}");
                let half = KeyHalf::decode(&key_code).unwrap();
                assert_eq!(half.mult, m);
                let (k, from_key) = half.combine(a, &request, Some(&back)).unwrap();
                assert_eq!(from_key, address);
                let want = scale_secret(a, m).unwrap().add_tweak(&Scalar::from_be_bytes(b.secret_bytes()).unwrap()).unwrap();
                assert_eq!(k, want);
                assert_eq!(find_codes(&format!("x {code} y {key_code}"), "VGPROOF"), vec![code.clone()]);
                assert_eq!(find_codes(&format!("x {code} y {key_code}"), "VGKEY"), vec![key_code.clone()]);
            }
        }
    }

    #[test]
    fn multiplier_cannot_be_swapped_or_faked() {
        let a = key(70);
        let request = Request::many(vec![PublicKey::from_secret_key(secp(), &a)]);
        let b = key(71);
        let good = Proof::signed_with(&request, 1, Network::Tron, &b, 3).unwrap();
        good.verify(&request).unwrap();
        for m in [0u8, 1, 2, 4, 5] {
            assert_eq!(Proof { mult: m, ..good.clone() }.verify(&request), Err(SplitError::BadSignature), "{m}");
        }
        let body = good.encode();
        let body = body.rsplit_once(':').unwrap().0;
        let fields: Vec<&str> = body.split(':').collect();
        for bad in ["0", "6", "9", "03", "-1", ""] {
            let mut f = fields.clone();
            f[4] = bad;
            assert!(Proof::decode(&with_checksum(f.join(":"))).is_err(), "{bad}");
        }
        let mut f = fields.clone();
        f[4] = "2";
        assert_eq!(Proof::decode(&with_checksum(f.join(":"))).unwrap().verify(&request), Err(SplitError::BadSignature));

        let half = KeyHalf { order_id: request.order_id(), index: Some(1), net: Network::Tron, claimed: None, b, mult: 3 };
        let other = KeyHalf { mult: 5, ..half.clone() };
        assert_eq!(other.combine(&a, &request, Some(&good)), Err(SplitError::ProofMismatch));
        assert_ne!(other.verify(&request).unwrap(), good.verify(&request).unwrap());
        let legacy = Request::single(PublicKey::from_secret_key(secp(), &a));
        let legacy_half = KeyHalf { order_id: legacy.order_id(), index: None, mult: 2, ..half.clone() };
        assert_eq!(legacy_half.verify(&legacy), Err(SplitError::BadFormat));
        assert!(Proof::signed_with(&request, 1, Network::Tron, &b, MULTIPLIERS).is_err());
    }
}
