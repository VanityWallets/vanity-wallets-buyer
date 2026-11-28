use crate::split_key::{Encoding, Network, RIPPLE_ALPHABET};

pub const B58: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
pub const BECH32: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
pub const HEX: &str = "0123456789abcdefABCDEF";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    Base58,
    Hex,
    Bech32,
}

impl Charset {
    pub fn of(net: Network) -> Charset {
        match net.encoding() {
            Encoding::Evm => Charset::Hex,
            Encoding::SegwitV0 { .. } | Encoding::Taproot | Encoding::Bech32 { .. } => Charset::Bech32,
            Encoding::Tron | Encoding::Base58 { .. } => Charset::Base58,
        }
    }

    pub fn alphabet(self) -> &'static str {
        match self {
            Charset::Base58 => B58,
            Charset::Hex => HEX,
            Charset::Bech32 => BECH32,
        }
    }

    pub fn bad_chars(self, s: &str) -> Vec<char> {
        let mut bad: Vec<char> = Vec::new();
        for c in s.chars().filter(|c| !self.alphabet().contains(*c)) {
            if !bad.contains(&c) {
                bad.push(c);
            }
        }
        bad
    }

    pub fn forbidden(self, ru: bool) -> &'static str {
        match (self, ru) {
            (Charset::Base58, true) => "0  O  I  l, а также пробелы, знаки препинания и любые буквы не латиницы",
            (Charset::Base58, false) => "0  O  I  l, as well as spaces, punctuation and any non-Latin letters",
            (Charset::Hex, true) => {
                "буквы g–z и G–Z (все, кроме a–f и A–F), а также пробелы, знаки препинания и буквы не латиницы"
            }
            (Charset::Hex, false) => {
                "letters g–z and G–Z (all except a–f and A–F), as well as spaces, punctuation and non-Latin letters"
            }
            (Charset::Bech32, true) => {
                "1  b  i  o  и все заглавные буквы A–Z, а также пробелы, знаки препинания и буквы не латиницы"
            }
            (Charset::Bech32, false) => {
                "1  b  i  o  and all capital letters A–Z, as well as spaces, punctuation and non-Latin letters"
            }
        }
    }

    pub fn allowed(self, ru: bool) -> &'static str {
        match (self, ru) {
            (Charset::Base58, true) => "1–9, A–Z и a–z, кроме перечисленных",
            (Charset::Base58, false) => "1–9, A–Z and a–z, except the ones listed",
            (Charset::Hex, true) => "0–9, a–f, A–F; регистр букв важен",
            (Charset::Hex, false) => "0–9, a–f, A–F; letter case matters",
            (Charset::Bech32, true) => "строчные a–z и цифры 0, 2–9, кроме b, i, o",
            (Charset::Bech32, false) => "lowercase a–z and digits 0, 2–9, except b, i, o",
        }
    }

    pub fn rules_text(self, ru: bool) -> String {
        if ru {
            format!("Запрещены: {}. Можно: {}.", self.forbidden(ru), self.allowed(ru))
        } else {
            format!("Forbidden: {}. Allowed: {}.", self.forbidden(ru), self.allowed(ru))
        }
    }
}

pub fn show_chars(bad: &[char], ru: bool) -> String {
    bad.iter()
        .map(|c| {
            if c.is_whitespace() {
                (if ru { "пробел" } else { "space" }).to_string()
            } else {
                format!("«{c}»")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn fixed_start(net: Network) -> &'static str {
    match net {
        Network::Tron => "T",
        Network::Eth
        | Network::Bsc
        | Network::Polygon
        | Network::Arbitrum
        | Network::Optimism
        | Network::Base
        | Network::Avalanche
        | Network::Linea
        | Network::ZkSync
        | Network::HyperEvm => "0x",
        Network::BtcP2pkh => "1",
        Network::BtcP2sh => "3",
        Network::BtcP2wpkh => "bc1q",
        Network::BtcP2tr => "bc1p",
        Network::LtcP2pkh => "L",
        Network::LtcP2sh => "M",
        Network::LtcP2wpkh => "ltc1q",
        Network::Doge => "D",
        Network::Dash => "X",
        Network::Xrp => "r",
        Network::Cosmos => "cosmos1",
        Network::Osmosis => "osmo1",
        Network::Celestia => "celestia1",
        Network::Dydx => "dydx1",
        Network::Akash => "akash1",
        Network::Juno => "juno1",
        Network::Stargaze => "stars1",
        Network::Noble => "noble1",
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Big(Vec<u32>);

impl Big {
    fn new(v: u32) -> Big {
        Big(vec![v])
    }

    fn shifted_192(v: u32) -> Big {
        Big(vec![0, 0, 0, 0, 0, 0, v])
    }

    fn mul_add(&mut self, m: u32, a: u32) {
        let mut carry = u64::from(a);
        for w in &mut self.0 {
            let x = u64::from(*w) * u64::from(m) + carry;
            *w = x as u32;
            carry = x >> 32;
        }
        if carry > 0 {
            self.0.push(carry as u32);
        }
    }

    fn times_58_pow(mut self, n: usize) -> Big {
        for _ in 0..n {
            self.mul_add(58, 0);
        }
        self
    }

    fn key(&self) -> Vec<u32> {
        let mut v = self.0.clone();
        while v.len() > 1 && v.last() == Some(&0) {
            v.pop();
        }
        v
    }
}

impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let (a, b) = (self.key(), other.key());
        a.len().cmp(&b.len()).then_with(|| a.iter().rev().cmp(b.iter().rev()))
    }
}

fn base58_params(net: Network) -> Option<(u8, &'static str)> {
    match net.encoding() {
        Encoding::Tron => Some((0x41, B58)),
        Encoding::Base58 { version, ripple, .. } => {
            let ripple_alpha = std::str::from_utf8(RIPPLE_ALPHABET).unwrap_or(B58);
            Some((version, if ripple { ripple_alpha } else { B58 }))
        }
        _ => None,
    }
}

pub fn start_possible(net: Network, full_start: &str) -> bool {
    let Some((version, alpha)) = base58_params(net) else { return true };
    let zero = alpha.chars().next().unwrap_or('1');
    let (rest, lo_n, hi_n) = if version == 0 {
        let Some(rest) = full_start.strip_prefix(zero) else { return false };
        if rest.starts_with(zero) {
            return false;
        }
        (rest, Big::new(1), Big::shifted_192(1))
    } else {
        (full_start, Big::shifted_192(u32::from(version)), Big::shifted_192(u32::from(version) + 1))
    };
    if rest.is_empty() {
        return true;
    }
    let mut p = Big::new(0);
    for c in rest.chars() {
        let Some(d) = alpha.find(c) else { return false };
        p.mul_add(58, d as u32);
    }
    let mut p1 = p.clone();
    p1.mul_add(1, 1);
    let k = rest.chars().count();
    (k.max(1)..=36).any(|len| {
        let lo_a = p.clone().times_58_pow(len - k);
        let hi_a = p1.clone().times_58_pow(len - k);
        let lo_b = Big::new(1).times_58_pow(len - 1);
        let hi_b = Big::new(1).times_58_pow(len);
        let lo = lo_a.max(lo_b).max(lo_n.clone());
        let hi = hi_a.min(hi_b).min(hi_n.clone());
        lo < hi
    })
}

pub fn next_chars(net: Network) -> Option<String> {
    let (_, alpha) = base58_params(net)?;
    let fixed = fixed_start(net);
    Some(
        alpha
            .chars()
            .filter(|c| start_possible(net, &format!("{fixed}{c}")))
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub fn impossible_start(net: Network, full_start: &str, ru: bool) -> Option<String> {
    if start_possible(net, full_start) {
        return None;
    }
    let fixed = fixed_start(net);
    let allowed = next_chars(net).unwrap_or_default();
    Some(if ru {
        format!("адрес не может начинаться с «{full_start}»: сразу после «{fixed}» бывают только {allowed}")
    } else {
        format!("an address cannot start with “{full_start}”: right after “{fixed}” only {allowed} occur")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn possible_starts_match_the_site() {
        for (net, expected) in [
            (Network::Tron, "9ABCDEFGHJKLMNPQRSTUVWXYZ"),
            (Network::BtcP2pkh, "23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"),
            (Network::BtcP2sh, "123456789ABCDEFGHJKLMNPQR"),
            (Network::LtcP2pkh, "KLMNPQRSTUVWXYZabcdefghi"),
            (Network::LtcP2sh, "789ABCDEFGHJKLMNPQRSTUVWX"),
            (Network::Doge, "56789ABCDEFGHJKLMNPQRSTU"),
            (Network::Dash, "abcdefghijkmnopqrstuvwxyz"),
            (Network::Xrp, "pshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz"),
        ] {
            assert_eq!(next_chars(net).unwrap().replace(' ', ""), expected, "{net:?}");
        }
        use secp256k1::{PublicKey, Secp256k1, SecretKey};
        let secp = Secp256k1::new();
        for n in 1..60u8 {
            let pk = PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&[n; 32]).unwrap());
            for net in Network::ALL {
                let a = crate::split_key::address(net, &pk);
                assert!(a.starts_with(fixed_start(net)), "{net:?}: {a}");
                assert!(Charset::of(net).bad_chars(&a[fixed_start(net).len()..]).is_empty(), "{net:?}: {a}");
                for len in [2, 5, 9] {
                    assert!(start_possible(net, &a[..len]), "{net:?}: {a}");
                }
            }
        }
        assert!(next_chars(Network::Eth).is_none() && start_possible(Network::Eth, "0xzz"));
        assert!(!start_possible(Network::Tron, "T1abc") && !start_possible(Network::BtcP2pkh, "11"));
        assert!(impossible_start(Network::Tron, "T1", true).unwrap().contains("9 A B C"));
        assert!(impossible_start(Network::Tron, "TV", true).is_none());
    }

    #[test]
    fn forbidden_lists() {
        assert_eq!(Charset::Base58.bad_chars("P0tl0 "), vec!['0', 'l', ' ']);
        assert_eq!(Charset::Hex.bad_chars("cafeXg"), vec!['X', 'g']);
        assert_eq!(Charset::Bech32.bad_chars("qqBq1"), vec!['B', '1']);
        assert!(Charset::Bech32.rules_text(true).contains("1  b  i  o"));
        assert_eq!(show_chars(&['0', ' '], false), "«0», space");
    }
}
