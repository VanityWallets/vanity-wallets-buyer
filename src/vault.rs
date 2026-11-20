use crate::split_key::{self, Network, Request};
use rand::rngs::OsRng;
use rand::RngCore;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use serde_json::{Map, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroize;

const FORMAT: &str = "vanity-wallets-buyer-order-3";
const FORMAT_V3_OLD: &str = "pokupatel-order-3";
const FORMAT_V2: &str = "pokupatel-order-2";
pub const MAX_COUNT: u32 = 100;
const INDEX_FILE: &str = "orders_index.txt";

struct Field {
    ru: &'static str,
    en: &'static str,
    old: &'static str,
}

impl Field {
    fn name(&self, ru: bool) -> &'static str {
        if ru {
            self.ru
        } else {
            self.en
        }
    }

    fn get<'a>(&self, o: &'a Map<String, Value>) -> Option<&'a Value> {
        [self.ru, self.en, self.old].into_iter().filter(|n| !n.is_empty()).find_map(|n| o.get(n))
    }
}

const F_FORMAT: Field = Field { ru: "Формат", en: "Format", old: "Format" };
const F_WARNING: Field = Field { ru: "Внимание", en: "Warning", old: "Note" };
const F_ID: Field = Field { ru: "Номер заявки", en: "Request number", old: "OrderId" };
const F_DATE: Field = Field { ru: "Дата создания", en: "Date created", old: "" };
const F_CREATED: Field = Field { ru: "Время создания (секунды Unix)", en: "Created (Unix seconds)", old: "Created" };
const F_NET: Field = Field { ru: "Сеть", en: "Network", old: "Network" };
const F_START: Field = Field { ru: "Начало адреса", en: "Address start", old: "WantedStart" };
const F_END: Field = Field { ru: "Конец адреса", en: "Address end", old: "WantedEnd" };
const F_COUNT: Field = Field { ru: "Количество адресов", en: "Number of addresses", old: "" };
const F_REQUEST: Field = Field {
    ru: "Код заявки (его можно отправлять продавцу)",
    en: "Request code (safe to send to the seller)",
    old: "Request",
};
const F_SECRET: Field = Field {
    ru: "Секретная половина ключа (НИКОМУ не передавать)",
    en: "Secret key half (NEVER share it)",
    old: "SecretHalf",
};
const F_PROOFS: Field = Field {
    ru: "Шаг 2 · проверенные адреса (коды VGPROOF1)",
    en: "Step 2 · verified addresses (VGPROOF1 codes)",
    old: "VerifiedProof",
};
const F_ASSEMBLED: Field = Field {
    ru: "Шаг 3 · собранные кошельки (адреса)",
    en: "Step 3 · assembled wallets (addresses)",
    old: "AssembledAddress",
};

fn warning(ru: bool) -> &'static str {
    if ru {
        "Это файл заявки на vanity-кошелёк с разделённым ключом. В нём ваша секретная половина ключа: никому \
         не передавайте файл (в том числе продавцу) и не удаляйте его до получения всех кошельков — без него \
         собрать заказанные кошельки невозможно. Продавцу отправляется только код заявки VGREQ1."
    } else {
        "This is a request file for a split-key vanity wallet. It holds your secret key half: never share the \
         file (including with the seller) and do not delete it until you receive all the wallets — without it \
         the ordered wallets cannot be assembled. Only the VGREQ1 request code is sent to the seller."
    }
}

#[derive(Clone, Debug)]
pub struct Wish {
    pub net: Network,
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub id: String,
    pub request: String,
    pub created: u64,
    pub wish: Option<Wish>,
    pub count: u32,
    pub verified_proofs: Vec<String>,
    pub assembled: Vec<String>,
    pub path: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub enum VaultError {
    Damaged,
    Exists,
    Io(String),
}

impl VaultError {
    pub fn text(&self, ru: bool) -> String {
        match (self, ru) {
            (VaultError::Damaged, true) => "Файл заявки повреждён или изменён.".into(),
            (VaultError::Damaged, false) => "The request file is damaged or was modified.".into(),
            (VaultError::Exists, true) => {
                "Файл с таким именем уже есть. Выберите другое имя: существующие файлы не перезаписываются, \
                 чтобы не потерять другую заявку."
                    .into()
            }
            (VaultError::Exists, false) => {
                "A file with this name already exists. Choose another name: existing files are never \
                 overwritten, so that no other request is lost."
                    .into()
            }
            (VaultError::Io(e), true) => format!("Ошибка файла: {e}"),
            (VaultError::Io(e), false) => format!("File error: {e}"),
        }
    }
}

fn io_err(e: io::Error) -> VaultError {
    if e.kind() == io::ErrorKind::AlreadyExists {
        VaultError::Exists
    } else {
        VaultError::Io(e.to_string())
    }
}

pub fn data_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        if cfg!(test) {
            return std::env::temp_dir().join(format!("vwb_test_data_{}", std::process::id()));
        }
        match std::env::var_os("LOCALAPPDATA") {
            Some(base) => PathBuf::from(base).join("VanityBuyer"),
            None => std::env::temp_dir().join("VanityBuyer"),
        }
    })
    .clone()
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
}

pub fn orders_dir() -> Option<PathBuf> {
    exe_dir().map(|d| d.join("orders"))
}

fn migrate_old_index() {
    let Some(old) = exe_dir().map(|d| d.join(INDEX_FILE)) else { return };
    if same_path(&old, &index_path()) {
        return;
    }
    let Ok(text) = fs::read_to_string(&old) else { return };
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !remember(Path::new(line)) {
            return;
        }
    }
    let _ = fs::remove_file(&old);
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

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn date(secs: u64) -> String {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{d:02}.{m:02}.{y}")
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

struct Contents {
    ru: bool,
    id: String,
    created: u64,
    net: Option<Network>,
    start: String,
    end: String,
    count: u32,
    request: String,
    secret: String,
    proofs: Vec<String>,
    assembled: Vec<String>,
}

impl Drop for Contents {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

enum Val<'a> {
    Str(&'a str),
    Num(u64),
    List(&'a [String]),
}

fn render(fields: &[(&str, Val)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(2048);
    out.extend_from_slice(b"{\n");
    for (i, (name, val)) in fields.iter().enumerate() {
        out.extend_from_slice(b"  ");
        let _ = serde_json::to_writer(&mut out, name);
        out.extend_from_slice(b": ");
        match val {
            Val::Str(s) => {
                let _ = serde_json::to_writer(&mut out, s);
            }
            Val::Num(n) => out.extend_from_slice(n.to_string().as_bytes()),
            Val::List(items) if items.is_empty() => out.extend_from_slice(b"[]"),
            Val::List(items) => {
                out.extend_from_slice(b"[\n");
                for (j, item) in items.iter().enumerate() {
                    out.extend_from_slice(b"    ");
                    let _ = serde_json::to_writer(&mut out, item);
                    out.extend_from_slice(if j + 1 < items.len() { b",\n" } else { b"\n" });
                }
                out.extend_from_slice(b"  ]");
            }
        }
        out.extend_from_slice(if i + 1 < fields.len() { b",\n" } else { b"\n" });
    }
    out.extend_from_slice(b"}\n");
    out
}

fn or_dash(s: &str) -> &str {
    if s.is_empty() {
        "—"
    } else {
        s
    }
}

impl Contents {
    fn to_bytes(&self) -> Vec<u8> {
        let ru = self.ru;
        let net = self.net.map_or("—", Network::label);
        let date = date(self.created);
        render(&[
            (F_FORMAT.name(ru), Val::Str(FORMAT)),
            (F_WARNING.name(ru), Val::Str(warning(ru))),
            (F_ID.name(ru), Val::Str(&self.id)),
            (F_DATE.name(ru), Val::Str(&date)),
            (F_NET.name(ru), Val::Str(net)),
            (F_START.name(ru), Val::Str(or_dash(&self.start))),
            (F_END.name(ru), Val::Str(or_dash(&self.end))),
            (F_COUNT.name(ru), Val::Num(u64::from(self.count))),
            (F_REQUEST.name(ru), Val::Str(&self.request)),
            (F_SECRET.name(ru), Val::Str(&self.secret)),
            (F_PROOFS.name(ru), Val::List(&self.proofs)),
            (F_ASSEMBLED.name(ru), Val::List(&self.assembled)),
            (F_CREATED.name(ru), Val::Num(self.created)),
        ])
    }

    fn from_text(text: &str) -> Option<Contents> {
        let mut value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        let parsed = (|| {
            let o = value.as_object()?;
            let format = F_FORMAT.get(o)?.as_str()?;
            if ![FORMAT, FORMAT_V3_OLD, FORMAT_V2].contains(&format) {
                return None;
            }
            let s = |f: &Field| -> String {
                let v = f.get(o).and_then(Value::as_str).unwrap_or("").trim();
                if matches!(v, "—" | "-" | "–") { String::new() } else { v.to_string() }
            };
            let list = |f: &Field| -> Vec<String> {
                match f.get(o) {
                    Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
                    Some(Value::String(one)) if !one.is_empty() => vec![one.clone()],
                    _ => Vec::new(),
                }
            };
            let net_text = s(&F_NET);
            Some(Contents {
                ru: o.contains_key(F_FORMAT.ru),
                id: s(&F_ID),
                created: F_CREATED.get(o).and_then(Value::as_u64).unwrap_or(0),
                net: Network::ALL.into_iter().find(|n| n.tag() == net_text || n.label() == net_text),
                start: s(&F_START),
                end: s(&F_END),
                count: F_COUNT
                    .get(o)
                    .and_then(Value::as_u64)
                    .map_or(1, |n| n.clamp(1, u64::from(MAX_COUNT)) as u32),
                request: s(&F_REQUEST),
                secret: s(&F_SECRET),
                proofs: list(&F_PROOFS),
                assembled: list(&F_ASSEMBLED),
            })
        })();
        if let Some(Value::String(secret)) = value
            .as_object_mut()
            .and_then(|o| [F_SECRET.ru, F_SECRET.en, F_SECRET.old].into_iter().find_map(|n| o.remove(n)))
            .as_mut()
        {
            secret.zeroize();
        }
        parsed
    }

    fn order(&self, path: PathBuf) -> Order {
        Order {
            id: self.id.clone(),
            request: self.request.clone(),
            created: self.created,
            wish: self.net.map(|net| Wish { net, start: self.start.clone(), end: self.end.clone() }),
            count: self.count,
            verified_proofs: self.proofs.clone(),
            assembled: self.assembled.clone(),
            path,
        }
    }
}

fn read_file(path: &Path) -> Option<Contents> {
    let mut text = fs::read_to_string(path).ok()?;
    let c = Contents::from_text(&text);
    text.zeroize();
    c.filter(|c| !c.id.is_empty() && !c.request.is_empty())
}

pub struct Draft {
    a: [u8; 32],
    pub id: String,
    request: String,
}

impl Drop for Draft {
    fn drop(&mut self) {
        self.a.zeroize();
    }
}

impl Draft {
    pub fn new() -> Draft {
        let secp = Secp256k1::new();
        loop {
            let mut a = [0u8; 32];
            OsRng.fill_bytes(&mut a);
            let Ok(key) = SecretKey::from_slice(&a) else {
                a.zeroize();
                continue;
            };
            let request = Request { a_pub: PublicKey::from_secret_key(&secp, &key) };
            return Draft { a, id: request.order_id(), request: request.encode() };
        }
    }

    pub fn file_name(&self, net: Network, ru: bool) -> String {
        let tag: String = net.tag().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        format!("{}_{}_{tag}.json", if ru { "Заявка" } else { "Request" }, self.id)
    }

    pub fn save(self, path: &Path, wish: &Wish, count: u32, ru: bool) -> Result<Order, VaultError> {
        let fixed_body = |s: &str| s.trim().to_string();
        let contents = Contents {
            ru,
            id: self.id.clone(),
            created: now(),
            net: Some(wish.net),
            start: fixed_body(&wish.start),
            end: fixed_body(&wish.end),
            count: count.clamp(1, MAX_COUNT),
            request: self.request.clone(),
            secret: split_key::hex(&self.a),
            proofs: Vec::new(),
            assembled: Vec::new(),
        };
        let mut bytes = contents.to_bytes();
        let written = write_new(path, &bytes).map_err(io_err);
        bytes.zeroize();
        written?;
        remember(path);
        Ok(contents.order(path.to_path_buf()))
    }
}

fn index_path() -> PathBuf {
    data_dir().join(INDEX_FILE)
}

const HIDDEN: char = '-';

fn index_lines() -> Vec<String> {
    fs::read_to_string(index_path())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

fn index_paths() -> Vec<PathBuf> {
    index_lines().into_iter().filter(|l| !l.starts_with(HIDDEN)).map(PathBuf::from).collect()
}

fn hidden_paths() -> Vec<PathBuf> {
    index_lines().into_iter().filter_map(|l| l.strip_prefix(HIDDEN).map(PathBuf::from)).collect()
}

fn write_index(lines: &[String]) -> bool {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let _ = fs::create_dir_all(data_dir());
    let tmp = index_path().with_extension("txt.tmp");
    let _ = fs::remove_file(&tmp);
    write_new(&tmp, text.as_bytes()).is_ok() && fs::rename(&tmp, index_path()).is_ok()
}

fn line_path(line: &str) -> &Path {
    Path::new(line.strip_prefix(HIDDEN).unwrap_or(line))
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    norm(a).to_string_lossy().eq_ignore_ascii_case(&norm(b).to_string_lossy())
}

pub fn shown_path(path: &Path) -> String {
    let s = path.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
}

static INDEX_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn remember(path: &Path) -> bool {
    let _guard = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut lines = index_lines();
    let known = lines.iter().any(|l| !l.starts_with(HIDDEN) && same_path(line_path(l), path));
    let hidden = lines.iter().any(|l| l.starts_with(HIDDEN) && same_path(line_path(l), path));
    if known && !hidden {
        return true;
    }
    lines.retain(|l| !(l.starts_with(HIDDEN) && same_path(line_path(l), path)));
    if !known {
        let full = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().map(|d| d.join(path)).unwrap_or_else(|_| path.to_path_buf())
        };
        lines.push(shown_path(&full));
    }
    write_index(&lines)
}

pub fn delete(order: &Order) -> Result<(), VaultError> {
    let path = &order.path;
    if path.exists() {
        let ours = read_file(path).is_some_and(|c| c.id == order.id);
        if !ours {
            return Err(VaultError::Damaged);
        }
        wipe(path).map_err(|e| VaultError::Io(e.to_string()))?;
    }
    let tmp = path.with_extension("json.tmp");
    if tmp.exists() {
        let _ = wipe(&tmp);
    }
    if forget(path) {
        Ok(())
    } else {
        Err(VaultError::Io("orders_index.txt".into()))
    }
}

fn wipe(path: &Path) -> io::Result<()> {
    let len = fs::metadata(path)?.len() as usize;
    {
        let mut f = OpenOptions::new().write(true).open(path)?;
        f.write_all(&vec![0u8; len])?;
        f.sync_all()?;
    }
    fs::remove_file(path)
}

pub fn forget(path: &Path) -> bool {
    let _guard = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut lines = index_lines();
    lines.retain(|l| !same_path(line_path(l), path));
    let legacy = orders_dir().is_some_and(|d| path.parent().is_some_and(|p| same_path(p, &d)));
    if legacy {
        lines.push(format!("{HIDDEN}{}", shown_path(path)));
    }
    write_index(&lines)
}

pub fn open(path: &Path) -> Result<Order, VaultError> {
    let c = read_file(path).ok_or(VaultError::Damaged)?;
    remember(path);
    Ok(c.order(path.to_path_buf()))
}

pub struct Listing {
    pub orders: Vec<Order>,
    pub missing: Vec<PathBuf>,
}

fn list_paths(known: Vec<PathBuf>, legacy: Vec<PathBuf>) -> Listing {
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut orders: Vec<Order> = Vec::new();
    let mut missing: Vec<PathBuf> = Vec::new();
    let count = known.len();
    for (i, path) in known.into_iter().chain(legacy).enumerate() {
        if seen.iter().any(|p| same_path(p, &path)) {
            continue;
        }
        seen.push(path.clone());
        match read_file(&path) {
            Some(c) => orders.push(c.order(path)),
            None if i < count => missing.push(path),
            None => {}
        }
    }
    orders.sort_by(|a, b| b.created.cmp(&a.created));
    Listing { orders, missing }
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect()
}

pub fn list() -> Listing {
    migrate_old_index();
    let hidden = hidden_paths();
    let legacy = orders_dir()
        .map(|d| json_files(&d))
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !hidden.iter().any(|h| same_path(h, p)))
        .collect();
    list_paths(index_paths(), legacy)
}

pub fn secret(order: &Order) -> Result<SecretKey, VaultError> {
    let c = read_file(&order.path).ok_or(VaultError::Damaged)?;
    let mut bytes = unhex(&c.secret).ok_or(VaultError::Damaged)?;
    let a = SecretKey::from_slice(&bytes).map_err(|_| VaultError::Damaged);
    bytes.zeroize();
    let a = a?;
    let request = Request::decode(&c.request).map_err(|_| VaultError::Damaged)?;
    if PublicKey::from_secret_key(&Secp256k1::new(), &a) != request.a_pub {
        return Err(VaultError::Damaged);
    }
    Ok(a)
}

pub fn secret_hint(order: &Order) -> Option<String> {
    let c = read_file(&order.path)?;
    let s = c.secret.trim();
    if s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(format!("{}…{}", &s[..6], &s[58..]))
    } else {
        None
    }
}

pub fn save_verified_proofs(order: &Order, proofs: &[String]) -> Result<(), VaultError> {
    update(order, |c| c.proofs = proofs.to_vec())
}

pub fn save_assembled(order: &Order, addresses: &[String]) -> Result<(), VaultError> {
    update(order, |c| {
        for a in addresses {
            if !c.assembled.contains(a) {
                c.assembled.push(a.clone());
            }
        }
    })
}

fn update(order: &Order, change: impl FnOnce(&mut Contents)) -> Result<(), VaultError> {
    let mut c = read_file(&order.path).ok_or(VaultError::Damaged)?;
    change(&mut c);
    let mut bytes = c.to_bytes();
    let tmp = order.path.with_extension("json.tmp");
    let _ = fs::remove_file(&tmp);
    let written = write_new(&tmp, &bytes).map_err(io_err);
    bytes.zeroize();
    written?;
    fs::rename(&tmp, &order.path).map_err(|e| VaultError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wish() -> Wish {
        Wish { net: Network::Tron, start: "TPet".into(), end: String::new() }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vwb_test_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_read_update_in_both_languages() {
        let dir = temp_dir("vault");
        for ru in [true, false] {
            let draft = Draft::new();
            let path = dir.join(draft.file_name(Network::Tron, ru));
            let order = draft.save(&path, &wish(), 3, ru).unwrap();
            let text = fs::read_to_string(&path).unwrap();
            let first = if ru { "\"Формат\"" } else { "\"Format\"" };
            assert!(text.trim_start().strip_prefix('{').unwrap().trim_start().starts_with(first), "{text}");
            assert!(text.contains(if ru { "Количество адресов" } else { "Number of addresses" }));
            assert!(text.find(F_ID.name(ru)).unwrap() < text.find(F_SECRET.name(ru)).unwrap());

            let listed = list_paths(vec![path.clone()], Vec::new()).orders;
            assert_eq!(listed.len(), 1);
            let o = &listed[0];
            assert_eq!((o.id.as_str(), o.count), (order.id.as_str(), 3));
            let w = o.wish.clone().unwrap();
            assert_eq!((w.net, w.start.as_str(), w.end.as_str()), (Network::Tron, "TPet", ""));

            let a = secret(o).unwrap();
            let request = Request::decode(&o.request).unwrap();
            assert_eq!(PublicKey::from_secret_key(&Secp256k1::new(), &a), request.a_pub);

            save_verified_proofs(o, &["VGPROOF1:one".into(), "VGPROOF1:two".into()]).unwrap();
            save_assembled(o, &["TAddr1".into()]).unwrap();
            save_assembled(o, &["TAddr1".into(), "TAddr2".into()]).unwrap();
            let o = &list_paths(vec![path.clone()], Vec::new()).orders[0];
            assert_eq!(o.verified_proofs, ["VGPROOF1:one", "VGPROOF1:two"]);
            assert_eq!(o.assembled, ["TAddr1", "TAddr2"]);
            assert!(secret(o).is_ok());
            assert!(fs::read_to_string(&path).unwrap().contains(F_FORMAT.name(ru)));
            assert_eq!(Draft::new().save(&path, &wish(), 1, ru).unwrap_err(), VaultError::Exists);
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_version_two_files() {
        let dir = temp_dir("v2");
        let draft = Draft::new();
        let v2 = format!(
            "{{\"Format\":\"pokupatel-order-2\",\"OrderId\":\"{}\",\"Request\":\"{}\",\"Created\":5,\
             \"SecretHalf\":\"{}\",\"Network\":\"BTC-P2PKH\",\"WantedStart\":\"Boa\",\"WantedEnd\":\"\",\
             \"VerifiedProof\":\"VGPROOF1:x\",\"Note\":\"n\"}}",
            draft.id,
            draft.request,
            split_key::hex(&draft.a)
        );
        let path = dir.join("old.json");
        fs::write(&path, v2).unwrap();
        let o = &list_paths(vec![path.clone()], Vec::new()).orders[0];
        assert_eq!((o.count, o.created), (1, 5));
        assert_eq!(o.verified_proofs, ["VGPROOF1:x"]);
        assert_eq!(o.wish.as_ref().unwrap().net, Network::BtcP2pkh);
        assert!(secret(o).is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn swapped_request_is_detected() {
        let dir = temp_dir("swap");
        let one = Draft::new().save(&dir.join("one.json"), &wish(), 1, true).unwrap();
        let two = Draft::new().save(&dir.join("two.json"), &wish(), 1, true).unwrap();
        let text = fs::read_to_string(&one.path).unwrap().replace(&one.request, &two.request);
        fs::write(&one.path, text).unwrap();
        let order = list_paths(vec![one.path.clone()], Vec::new()).orders.remove(0);
        assert_eq!(secret(&order).unwrap_err(), VaultError::Damaged);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cancelled_order_file_is_deleted() {
        let dir = temp_dir("cancel");
        let order = Draft::new().save(&dir.join("cancel.json"), &wish(), 1, true).unwrap();
        let tmp = order.path.with_extension("json.tmp");
        fs::write(&tmp, "x").unwrap();
        delete(&order).unwrap();
        assert!(!order.path.exists() && !tmp.exists());
        assert!(!list().orders.iter().any(|o| o.id == order.id));
        assert!(!list().missing.iter().any(|p| same_path(p, &order.path)));

        let other = Draft::new().save(&dir.join("other.json"), &wish(), 1, true).unwrap();
        let mut fake = other.clone();
        fake.id = "00000000".into();
        assert_eq!(delete(&fake).unwrap_err(), VaultError::Damaged);
        assert!(other.path.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_files_are_listed_and_can_be_forgotten() {
        let dir = temp_dir("missing");
        let order = Draft::new().save(&dir.join("gone.json"), &wish(), 1, true).unwrap();
        fs::remove_file(&order.path).unwrap();
        let listing = list();
        assert!(listing.missing.iter().any(|p| same_path(p, &order.path)));
        assert!(forget(&order.path));
        assert!(!list().missing.iter().any(|p| same_path(p, &order.path)));
        fs::remove_dir_all(&dir).unwrap();
    }
}
