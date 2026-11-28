use crate::patterns::{self, fixed_start, Charset};
use crate::split_key::{self, Encoding, KeyHalf, Network, Proof, Request, SplitError};
use crate::vault::{self, Wish};
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use zeroize::Zeroize;

const MAX_PART: usize = 20;

pub fn date(secs: u64) -> String {
    vault::date(secs)
}

fn ru_word(n: usize, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
    let (d, h) = (n % 10, n % 100);
    if d == 1 && h != 11 {
        one
    } else if (2..=4).contains(&d) && !(12..=14).contains(&h) {
        few
    } else {
        many
    }
}

const LANG_FILE: &str = "language.txt";

fn saved_ru() -> bool {
    cfg!(test) || std::fs::read_to_string(vault::data_dir().join(LANG_FILE)).map_or(true, |t| t.trim() != "en")
}

fn save_ru(ru: bool) {
    if cfg!(test) {
        return;
    }
    let dir = vault::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(LANG_FILE), if ru { "ru" } else { "en" });
}

fn peek(s: &str) -> String {
    if s.len() <= 12 {
        "…".to_string()
    } else {
        format!("{}…{}", &s[..6], &s[s.len() - 6..])
    }
}

fn short(s: &str) -> String {
    if s.len() <= 20 {
        s.to_string()
    } else {
        format!("{}…{}", &s[..8], &s[s.len() - 8..])
    }
}

pub fn typical_len(net: Network) -> usize {
    match net.encoding() {
        Encoding::Tron | Encoding::Base58 { .. } => 34,
        Encoding::Evm => 42,
        Encoding::SegwitV0 { hrp } => hrp.len() + 40,
        Encoding::Taproot => 62,
        Encoding::Bech32 { hrp } => hrp.len() + 39,
    }
}

pub fn wanted_body(net: Network, start: &str) -> &str {
    let s = start.trim();
    let fixed = fixed_start(net);
    s.strip_prefix(fixed)
        .or_else(|| if net.is_evm() { s.strip_prefix("0X") } else { None })
        .unwrap_or(s)
}

#[cfg(test)]
fn charset_ok(net: Network, s: &str) -> bool {
    Charset::of(net).bad_chars(s).is_empty()
}

pub fn wish_problem(net: Network, start: &str, end: &str, ru: bool) -> Option<String> {
    if start.is_empty() && end.is_empty() {
        return Some(if ru { "Укажите начало или конец адреса." } else { "Enter the address start or end." }.into());
    }
    let cs = Charset::of(net);
    let mut bad = cs.bad_chars(start);
    for c in cs.bad_chars(end) {
        if !bad.contains(&c) {
            bad.push(c);
        }
    }
    if !bad.is_empty() {
        return Some(if ru {
            format!(
                "Эти символы запрещены: {}.\nВ сети {} {}",
                patterns::show_chars(&bad, ru),
                net.label(),
                cs.rules_text(ru)
            )
        } else {
            format!(
                "These characters are forbidden: {}.\nIn {}: {}",
                patterns::show_chars(&bad, ru),
                net.label(),
                cs.rules_text(ru)
            )
        });
    }
    if start.chars().count() > MAX_PART || end.chars().count() > MAX_PART {
        return Some(if ru {
            format!("Слишком длинно: в начале и в конце адреса — не больше {MAX_PART} символов.")
        } else {
            format!("Too long: at most {MAX_PART} characters at the start and at the end of the address.")
        });
    }
    if !start.is_empty() {
        let full = format!("{}{start}", fixed_start(net));
        if let Some(why) = patterns::impossible_start(net, &full, ru) {
            return Some(format!("{}: {why}.", net.label()));
        }
    }
    None
}

pub fn wish_matches(wish: &Wish, address: &str) -> bool {
    let fixed = fixed_start(wish.net);
    let body = address.strip_prefix(fixed).unwrap_or(address);
    body.starts_with(wanted_body(wish.net, &wish.start)) && address.ends_with(wish.end.trim())
}

fn wish_text(wish: &Wish, ru: bool) -> String {
    let fixed = fixed_start(wish.net);
    let start = wanted_body(wish.net, &wish.start);
    let end = wish.end.trim();
    let mut parts = Vec::new();
    if !start.is_empty() {
        parts.push(if ru {
            format!("начинается с «{fixed}{start}»")
        } else {
            format!("starts with “{fixed}{start}”")
        });
    }
    if !end.is_empty() {
        parts.push(if ru { format!("заканчивается на «{end}»") } else { format!("ends with “{end}”") });
    }
    parts.join(if ru { " и " } else { " and " })
}

pub fn wish_marks(wish: Option<&Wish>, address: &str) -> (usize, usize) {
    let Some(w) = wish.filter(|w| wish_matches(w, address)) else { return (0, 0) };
    let start = wanted_body(w.net, &w.start);
    let head = if start.is_empty() { 0 } else { fixed_start(w.net).len() + start.len() };
    let tail = w.end.trim().len();
    if head + tail > address.len() {
        (address.len(), 0)
    } else {
        (head, tail)
    }
}

fn address_view(wish: Option<&Wish>, address: &str) -> Value {
    let (head, tail) = wish_marks(wish, address);
    let mid_end = address.len() - tail;
    json!({
        "full": address,
        "head": &address[..head],
        "mid": &address[head..mid_end],
        "tail": &address[mid_end..],
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Ok,
    Warn,
    Fail,
}

struct Check {
    mark: Mark,
    text: String,
}

struct Outcome {
    checks: Vec<Check>,
    passed: bool,
    summary: String,
    found: Vec<(Network, String)>,
}

impl Outcome {
    fn new() -> Self {
        Outcome { checks: Vec::new(), passed: true, summary: String::new(), found: Vec::new() }
    }

    fn ok(&mut self, text: String) {
        self.checks.push(Check { mark: Mark::Ok, text });
    }

    fn warn(&mut self, text: String) {
        self.checks.push(Check { mark: Mark::Warn, text });
    }

    fn fail(&mut self, text: String) {
        self.checks.push(Check { mark: Mark::Fail, text });
        self.passed = false;
    }

    fn view(&self, wish: Option<&Wish>) -> Value {
        json!({
            "passed": self.passed,
            "summary": self.summary,
            "found": self.found.iter().map(|(_, a)| address_view(wish, a)).collect::<Vec<_>>(),
            "net": self.found.first().map(|(n, _)| n.label()),
            "checks": self.checks.iter().map(|c| json!({
                "mark": match c.mark { Mark::Ok => "ok", Mark::Warn => "warn", Mark::Fail => "fail" },
                "text": c.text,
            })).collect::<Vec<_>>(),
        })
    }
}

fn decode_error_text(e: SplitError, tag: &str, ru: bool) -> String {
    match (e, ru) {
        (SplitError::BadFormat, true) => format!("Это не тот код: нужен код, который начинается с {tag}. Скопируйте его целиком."),
        (SplitError::BadFormat, false) => format!("Wrong code: you need the code that starts with {tag}. Copy it in full."),
        (SplitError::BadChecksum, true) => {
            "Код скопирован с ошибкой или не полностью (контрольная сумма в конце не сошлась). Скопируйте его заново.".into()
        }
        (SplitError::BadChecksum, false) => {
            "The code was copied with a mistake or not in full (the checksum at the end does not match). Copy it again.".into()
        }
        (e, ru) => e.text(ru).to_string(),
    }
}

struct Wallet {
    net: Network,
    address: String,
    secret: String,
    a_short: String,
    b_short: String,
    k_short: String,
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.secret.zeroize();
        self.a_short.zeroize();
        self.k_short.zeroize();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Request,
    Verify,
    Wallet,
}

enum Msg {
    Ok(String),
    Err(String),
}

fn msg_view(m: &Option<Msg>) -> Value {
    match m {
        Some(Msg::Ok(t)) => json!({ "ok": true, "text": t }),
        Some(Msg::Err(t)) => json!({ "ok": false, "text": t }),
        None => Value::Null,
    }
}

pub enum Pick {
    SaveRequest { draft: vault::Draft, wish: Wish },
    OpenRequest,
    SaveWallets,
    Locate(PathBuf),
}

pub struct DialogRequest {
    pub save: bool,
    pub title: String,
    pub file_name: String,
    pub start_dir: Option<PathBuf>,
    pub filter: String,
}

pub enum Effect {
    None,
    CopyText(String),
    CopySecret(String),
    Dialog(DialogRequest),
}

pub struct BuyerApp {
    pub ru: bool,
    step: Step,
    orders: Vec<vault::Order>,
    selected: usize,
    new_mode: bool,
    form_epoch: u32,
    create_msg: Option<Msg>,
    copy_msg: Option<Msg>,
    open_msg: Option<Msg>,
    verify: Option<Outcome>,
    assemble: Option<Outcome>,
    wallets: Vec<Wallet>,
    wallet_msg: Option<Msg>,
    show_secret: bool,
    pub copied: Option<String>,
    pick: Option<Pick>,
    last_dir: Option<PathBuf>,
    hints: std::cell::RefCell<std::collections::HashMap<PathBuf, Option<String>>>,
    missing: Vec<PathBuf>,
    last_proofs: String,
    last_keys: zeroize::Zeroizing<String>,
    wallets_saved: bool,
    closing: bool,
}

impl BuyerApp {
    pub fn new() -> Self {
        let vault::Listing { orders, missing } = vault::list();
        let mut app = BuyerApp {
            missing,
            last_proofs: String::new(),
            last_keys: Default::default(),
            wallets_saved: false,
            closing: false,
            ru: saved_ru(),
            step: Step::Request,
            last_dir: orders.first().and_then(|o| o.path.parent().map(Path::to_path_buf)),
            new_mode: orders.is_empty(),
            orders,
            selected: 0,
            form_epoch: 0,
            create_msg: None,
            copy_msg: None,
            open_msg: None,
            verify: None,
            assemble: None,
            wallets: Vec::new(),
            wallet_msg: None,
            show_secret: false,
            copied: None,
            pick: None,
            hints: Default::default(),
        };
        app.step = app.next_step();
        app
    }

    fn t(&self, ru: &'static str, en: &'static str) -> &'static str {
        if self.ru {
            ru
        } else {
            en
        }
    }

    fn reload(&mut self, select_id: Option<&str>) {
        let vault::Listing { orders, missing } = vault::list();
        self.orders = orders;
        self.missing = missing;
        self.selected = select_id
            .and_then(|id| self.orders.iter().position(|o| o.id == id))
            .unwrap_or(0);
    }

    fn order(&self) -> Option<&vault::Order> {
        if self.new_mode {
            None
        } else {
            self.orders.get(self.selected)
        }
    }

    fn progress(&self) -> [bool; 3] {
        match self.order() {
            Some(o) => [true, o.verified_proofs.len() >= o.count as usize, o.assembled.len() >= o.count as usize],
            None => [false, false, false],
        }
    }

    fn next_step(&self) -> Step {
        match self.progress() {
            [false, _, _] => Step::Request,
            [true, false, _] => Step::Verify,
            _ => Step::Wallet,
        }
    }

    fn select_order(&mut self, index: usize) {
        self.selected = index;
        self.new_mode = false;
        self.verify = None;
        self.assemble = None;
        self.copy_msg = None;
        self.wallets.clear();
        self.show_secret = false;
        self.wallet_msg = None;
        self.last_proofs.clear();
        self.last_keys.zeroize();
    }

    fn dialog(&self, save: bool, title: &str, file_name: String) -> Effect {
        Effect::Dialog(DialogRequest {
            save,
            title: title.to_string(),
            file_name,
            start_dir: self.last_dir.clone(),
            filter: self.t("Файлы JSON (*.json)", "JSON files (*.json)").to_string(),
        })
    }

    pub fn act(&mut self, a: &Value) -> Effect {
        let s = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let n = |k: &str| a.get(k).and_then(Value::as_u64).unwrap_or(0);
        let busy = self.pick.is_some();
        match a.get("a").and_then(Value::as_str).unwrap_or("") {
            "lang" => {
                self.ru = a.get("ru").and_then(Value::as_bool).unwrap_or(true);
                save_ru(self.ru);
                self.create_msg = None;
                self.copy_msg = None;
                self.open_msg = None;
                self.wallet_msg = None;
                if self.verify.is_some() {
                    let text = std::mem::take(&mut self.last_proofs);
                    self.verify_proof(&text);
                }
                if self.assemble.is_some() {
                    let show = self.show_secret;
                    let saved = self.wallets_saved;
                    let text = self.last_keys.clone();
                    self.assemble(&text);
                    self.show_secret = show && !self.wallets.is_empty();
                    self.wallets_saved = saved;
                }
            }
            "step" => {
                self.step = match n("i") {
                    0 => Step::Request,
                    1 => Step::Verify,
                    _ => Step::Wallet,
                }
            }
            "select" => {
                let i = n("i") as usize;
                if i < self.orders.len() {
                    self.select_order(i);
                    self.step = self.next_step();
                }
            }
            "new" if !busy => {
                self.new_mode = true;
                self.step = Step::Request;
                self.create_msg = None;
            }
            "cancel_new" if !busy && !self.orders.is_empty() => {
                self.new_mode = false;
                self.create_msg = None;
                self.step = self.next_step();
            }
            "open_file" if !busy => {
                self.pick = Some(Pick::OpenRequest);
                self.open_msg = None;
                return self.dialog(false, self.t("Открыть файл заявки", "Open request file"), String::new());
            }
            "create" if !busy => {
                let net = Network::from_tag(&s("net")).unwrap_or(Network::Tron);
                let count = (n("count") as u32).clamp(1, vault::MAX_COUNT);
                return self.create_request(net, &s("start"), &s("end"), count);
            }
            "copy_message" => {
                if let Some(o) = self.order() {
                    let text = seller_message(o, self.ru);
                    self.copy_msg = Some(Msg::Ok(self.t("Сообщение скопировано.", "Message copied.").into()));
                    return Effect::CopyText(text);
                }
            }
            "verify" => self.verify_proof(&s("text")),
            "assemble" => {
                let text = zeroize::Zeroizing::new(s("text"));
                self.assemble(&text);
            }
            "toggle_keys" => self.show_secret = !self.show_secret,
            "close_wallets" => {
                self.wallets.clear();
                self.show_secret = false;
                self.wallet_msg = None;
                self.assemble = None;
                self.last_keys.zeroize();
            }
            "locate" if !busy => {
                if let Some(old) = self.missing.get(n("i") as usize).cloned() {
                    self.open_msg = None;
                    let start_dir = old.parent().filter(|d| d.is_dir()).map(Path::to_path_buf).or_else(|| self.last_dir.clone());
                    let file_name = old.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    self.pick = Some(Pick::Locate(old));
                    return Effect::Dialog(DialogRequest {
                        save: false,
                        title: self.t("Найти файл заявки", "Find the request file").into(),
                        file_name,
                        start_dir,
                        filter: self.t("Файлы JSON (*.json)", "JSON files (*.json)").into(),
                    });
                }
            }
            "forget_missing" if !busy => {
                if let Some(old) = self.missing.get(n("i") as usize).cloned() {
                    vault::forget(&old);
                    let id = self.order().map(|o| o.id.clone());
                    self.reload(id.as_deref());
                }
            }
            "cancel_order" if !busy => {
                let id = s("id");
                if let Some(o) = self.orders.iter().find(|o| o.id == id).cloned() {
                    let current = self.order().map(|c| c.id.clone()).filter(|c| *c != o.id);
                    match vault::delete(&o) {
                        Ok(()) => {
                            self.hints.borrow_mut().remove(&o.path);
                            self.reload(current.as_deref());
                            if current.is_none() {
                                self.select_order(0);
                                self.new_mode = self.orders.is_empty();
                                self.step = self.next_step();
                            }
                            self.open_msg = Some(Msg::Ok(format!("{} #{}", self.t("Заказ отменён, файл заявки удалён:", "Order cancelled, request file deleted:"), o.id)));
                        }
                        Err(e) => self.open_msg = Some(Msg::Err(e.text(self.ru))),
                    }
                }
            }
            "close_cancel" => self.closing = false,
            "save_wallets" if !busy && !self.wallets.is_empty() => {
                self.closing = false;
                self.pick = Some(Pick::SaveWallets);
                self.wallet_msg = None;
                let id = self.orders.get(self.selected).map(|o| o.id.clone()).unwrap_or_default();
                let name = format!("{}_{id}.json", self.t("Кошельки", "Wallets"));
                return self.dialog(true, self.t("Куда сохранить кошельки", "Where to save the wallets"), name);
            }
            "copy_address" => {
                if let Some(w) = self.wallets.get(n("i") as usize) {
                    self.wallet_msg = Some(Msg::Ok(self.t("Адрес скопирован.", "Address copied.").into()));
                    return Effect::CopyText(w.address.clone());
                }
            }
            "copy_found" => {
                let addr = self.verify.as_ref().and_then(|v| v.found.get(n("i") as usize)).map(|(_, a)| a.clone());
                if let Some(a) = addr {
                    return Effect::CopyText(a);
                }
            }
            "copy_key" => {
                if let Some(w) = self.wallets.get(n("i") as usize) {
                    return Effect::CopySecret(w.secret.clone());
                }
            }
            _ => {}
        }
        Effect::None
    }

    pub fn key_copied(&mut self, mut secret: String, ok: bool) {
        self.wallet_msg = Some(if ok {
            if let Some(mut old) = self.copied.replace(secret.clone()) {
                old.zeroize();
            }
            Msg::Ok(self.t(
                "Ключ скопирован. При выходе из программы он будет стёрт из буфера обмена.",
                "Key copied. It is wiped from the clipboard when the program exits.",
            ).into())
        } else {
            Msg::Err(self.t("Не удалось скопировать ключ.", "Could not copy the key.").into())
        });
        secret.zeroize();
    }

    fn create_request(&mut self, net: Network, start: &str, end: &str, count: u32) -> Effect {
        let ru = self.ru;
        let start = wanted_body(net, start).to_string();
        let end = end.trim().to_string();
        if let Some(problem) = wish_problem(net, &start, &end, ru) {
            self.create_msg = Some(Msg::Err(problem));
            return Effect::None;
        }
        let full_start = if start.is_empty() { String::new() } else { format!("{}{start}", fixed_start(net)) };
        let draft = vault::Draft::new(count);
        let name = draft.file_name(net, ru);
        self.create_msg = None;
        self.pick = Some(Pick::SaveRequest { draft, wish: Wish { net, start: full_start, end } });
        self.dialog(true, self.t("Куда сохранить файл заявки", "Where to save the request file"), name)
    }

    pub fn pick_done(&mut self, chosen: Option<PathBuf>) {
        let Some(pick) = self.pick.take() else { return };
        if let Some(dir) = chosen.as_deref().and_then(Path::parent) {
            self.last_dir = Some(dir.to_path_buf());
        }
        let ru = self.ru;
        match pick {
            Pick::SaveRequest { draft, wish } => {
                let Some(path) = chosen else {
                    self.create_msg = Some(Msg::Err(self.t(
                        "Заявка не создана: место для файла не выбрано.",
                        "The request was not created: no place for the file was chosen.",
                    ).into()));
                    return;
                };
                match draft.save(&path, &wish, ru) {
                    Ok(order) => {
                        self.reload(Some(&order.id));
                        let i = self.selected;
                        self.select_order(i);
                        self.form_epoch += 1;
                        self.create_msg = None;
                        self.copy_msg = Some(Msg::Ok(format!(
                            "{} {}",
                            self.t("Файл заявки сохранён:", "Request file saved:"),
                            vault::shown_path(&path)
                        )));
                    }
                    Err(e) => self.create_msg = Some(Msg::Err(e.text(ru))),
                }
            }
            Pick::OpenRequest => {
                let Some(path) = chosen else { return };
                match vault::open(&path) {
                    Ok(order) => {
                        self.reload(Some(&order.id));
                        let i = self.selected;
                        self.select_order(i);
                        self.step = self.next_step();
                        self.open_msg = Some(Msg::Ok(format!("{} #{}", self.t("Заявка открыта:", "Request opened:"), order.id)));
                    }
                    Err(_) => {
                        self.open_msg = Some(Msg::Err(self.t(
                            "Это не файл заявки этой программы или он повреждён.",
                            "This is not a request file of this program, or it is damaged.",
                        ).into()))
                    }
                }
            }
            Pick::SaveWallets => {
                if let Some(path) = chosen {
                    self.save_wallets(&path);
                }
            }
            Pick::Locate(old) => {
                let Some(path) = chosen else { return };
                match vault::open(&path) {
                    Ok(order) => {
                        vault::forget(&old);
                        vault::remember(&path);
                        self.reload(Some(&order.id));
                        let i = self.selected;
                        self.select_order(i);
                        self.step = self.next_step();
                        self.open_msg = Some(Msg::Ok(format!("{} #{}", self.t("Файл найден:", "File found:"), order.id)));
                    }
                    Err(_) => {
                        self.open_msg = Some(Msg::Err(self.t(
                            "Это не файл заявки этой программы или он повреждён.",
                            "This is not a request file of this program, or it is damaged.",
                        ).into()))
                    }
                }
            }
        }
    }

    fn check_net(&self, out: &mut Outcome, order: &vault::Order, net: Network) {
        let Some(wish) = &order.wish else { return };
        if wish.net == net {
            out.ok(format!("{} {} — {}", self.t("Сеть:", "Network:"), net.label(),
                self.t("как вы заказывали.", "as you ordered.")));
        } else {
            out.fail(format!("{} {}, {} {}.", self.t("В кодах сеть", "The codes have network"), net.label(),
                self.t("а вы заказывали", "but you ordered"), wish.net.label()));
        }
    }

    fn address_fits(order: &vault::Order, address: &str) -> Option<bool> {
        order.wish.as_ref().map(|w| wish_matches(w, address))
    }

    fn codes_from_input(&self, out: &mut Outcome, text: &str, kind: &str) -> Vec<String> {
        let codes = split_key::find_codes(text, kind);
        if codes.is_empty() {
            out.fail(decode_error_text(SplitError::BadFormat, kind, self.ru));
        }
        codes
    }

    fn order_of_codes(&mut self, out: &mut Outcome, ids: &[String]) -> Option<vault::Order> {
        let first = ids.first()?;
        if ids.iter().any(|id| id != first) {
            out.fail(self.t(
                "Коды относятся к разным заявкам. Вставьте коды только одной заявки.",
                "The codes belong to different requests. Paste the codes of one request only.",
            ).into());
            return None;
        }
        let Some(pos) = self.orders.iter().position(|o| &o.id == first) else {
            out.fail(format!("{} {first} {}", self.t("Коды относятся к заявке", "The codes belong to request"),
                self.t("— её нет среди заявок программы. Откройте её файл кнопкой «Открыть файл заявки…».",
                    "— it is not among the program's requests. Open its file with “Open request file…”.")));
            return None;
        };
        if self.new_mode || pos != self.selected {
            self.selected = pos;
            self.new_mode = false;
        }
        let order = self.orders[pos].clone();
        out.ok(format!("{} #{}.", self.t("Коды относятся к вашей заявке", "The codes belong to your request"), order.id));
        Some(order)
    }

    fn verify_proof(&mut self, input: &str) {
        self.last_proofs = input.to_string();
        let ru = self.ru;
        let mut out = Outcome::new();
        'checks: {
            let codes = self.codes_from_input(&mut out, input, "VGPROOF");
            if codes.is_empty() {
                break 'checks;
            }
            let mut proofs: Vec<Proof> = Vec::new();
            for (i, code) in codes.iter().enumerate() {
                match Proof::decode(code) {
                    Ok(p) => proofs.push(p),
                    Err(e) => out.fail(format!("{} {}: {}", self.t("Код", "Code"), i + 1, decode_error_text(e, "VGPROOF", ru))),
                }
            }
            if !out.passed {
                break 'checks;
            }
            out.ok(match (proofs.len(), ru) {
                (1, true) => "Код цел: контрольная сумма сошлась, он скопирован без ошибок.".into(),
                (1, false) => "The code is intact: the checksum matches, it was copied without mistakes.".into(),
                (n, true) => format!("Кодов: {n}. Все целы: контрольные суммы сошлись, скопированы без ошибок."),
                (n, false) => format!("Codes: {n}. All intact: the checksums match, copied without mistakes."),
            });
            let ids: Vec<String> = proofs.iter().map(|p| p.order_id.clone()).collect();
            let Some(order) = self.order_of_codes(&mut out, &ids) else { break 'checks };
            let Ok(request) = Request::decode(&order.request) else {
                out.fail(self.t("Файл заявки повреждён.", "The request file is damaged.").into());
                break 'checks;
            };
            if proofs.iter().any(|p| p.net != proofs[0].net) {
                out.fail(self.t("В кодах разные сети.", "The codes have different networks.").into());
            }
            self.check_net(&mut out, &order, proofs[0].net);

            let mut seen: Vec<String> = Vec::new();
            let mut seen_index: Vec<u32> = Vec::new();
            let many = proofs.len() > 1;
            let mut fit = 0usize;
            let wanted = order.wish.as_ref().map(|w| wish_text(w, ru)).unwrap_or_default();
            if request.legacy && order.count > 1 {
                out.warn(self.t(
                    "Заявка создана старой версией программы: у всех её адресов одна и та же ваша половина ключа. Для новых заказов создайте новую заявку.",
                    "The request was made by an old version of the program: all its addresses share one key half of yours. Create a new request for new orders.",
                ).into());
            }
            for (i, p) in proofs.iter().enumerate() {
                let num = if proofs.len() > 1 { format!("{}. ", i + 1) } else { String::new() };
                if let Some(index) = p.index {
                    if seen_index.contains(&index) {
                        out.fail(format!("{num}{} {index} {}", self.t("Для адреса №", "For address no."), self.t(
                            "прислано два кода — каждый адрес должен иметь свою половину.",
                            "two codes were sent — every address must have its own half.")));
                        continue;
                    }
                    seen_index.push(index);
                }
                let address = match p.verify(&request) {
                    Ok(a) => a,
                    Err(SplitError::AddressMismatch) => {
                        out.fail(format!("{num}{}", self.t(
                            "В коде прошлой версии указан адрес, который НЕ получается из вашей половины — продавец прислал чужой адрес.",
                            "The old-version code names an address that does NOT come from your half — the seller sent a foreign address.",
                        )));
                        continue;
                    }
                    Err(e) => {
                        out.fail(format!("{num}{}", e.text(ru)));
                        continue;
                    }
                };
                if seen.contains(&address) {
                    out.fail(format!("{num}{} {address} {}", self.t("Адрес", "Address"), self.t("прислан дважды.", "was sent twice.")));
                    continue;
                }
                seen.push(address.clone());
                match Self::address_fits(&order, &address) {
                    Some(false) => out.fail(format!(
                        "{num}{} {address}, {} {wanted}.",
                        self.t("Из половин получается адрес", "The halves give address"),
                        self.t("но он НЕ подходит под заказ, вы хотели:", "but it does NOT match your order, you wanted:")
                    )),
                    _ if many => fit += 1,
                    Some(true) => out.ok(format!(
                        "{} {address} — {} {wanted}.",
                        self.t("Из вашей половины и половины продавца получается адрес", "Your half and the seller's half give address"),
                        self.t("подходит под заказ:", "matches your order:")
                    )),
                    None => out.ok(format!(
                        "{} {address}.",
                        self.t("Из вашей половины и половины продавца получается адрес", "Your half and the seller's half give address")
                    )),
                }
            }
            if fit > 0 {
                let all = fit == proofs.len();
                let matches = order.wish.is_some();
                out.ok(match (ru, all, matches) {
                    (true, true, true) => format!("Все {fit} {} вычислены из вашей половины и половины продавца и подходят под заказ: {wanted}.", ru_word(fit, "адрес", "адреса", "адресов")),
                    (true, true, false) => format!("Все {fit} {} вычислены из вашей половины и половины продавца.", ru_word(fit, "адрес", "адреса", "адресов")),
                    (true, false, _) => format!("{fit} из {} адресов вычислены из вашей половины и половины продавца и подходят под заказ.", proofs.len()),
                    (false, true, true) => format!("All {fit} addresses are computed from your half and the seller's half and match your order: {wanted}."),
                    (false, true, false) => format!("All {fit} addresses are computed from your half and the seller's half."),
                    (false, false, _) => format!("{fit} of {} addresses are computed from your half and the seller's half and match your order.", proofs.len()),
                });
            }

            let got = seen.len();
            let wanted = order.count as usize;
            match got.cmp(&wanted) {
                std::cmp::Ordering::Equal => out.ok(if ru {
                    format!("Адресов: {got} — ровно столько, сколько вы заказали.")
                } else {
                    format!("Addresses: {got} — exactly as many as you ordered.")
                }),
                std::cmp::Ordering::Less => out.fail(if ru {
                    format!("Прислано адресов: {got} из {wanted} заказанных. Попросите продавца прислать все коды.")
                } else {
                    format!("Addresses received: {got} of the {wanted} ordered. Ask the seller to send all the codes.")
                }),
                std::cmp::Ordering::Greater => out.fail(if ru {
                    format!("Прислано адресов: {got}, а заказано {wanted}.")
                } else {
                    format!("Addresses received: {got}, but {wanted} were ordered.")
                }),
            }
            if out.passed {
                let codes: Vec<String> = proofs.iter().map(Proof::encode).collect();
                if let Err(e) = vault::save_verified_proofs(&order, &codes) {
                    out.fail(e.text(ru));
                }
                self.reload(Some(&order.id));
                out.found = seen.iter().map(|a| (proofs[0].net, a.clone())).collect();
            }
        }
        let many = out.found.len() > 1;
        out.summary = match (out.passed, many) {
            (true, false) => self.t(
                "Адрес вычислен программой из вашей половины ключа — управлять им сможете только вы: продавцу не хватает \
                 вашей секретной половины. Деньги на сам адрес не отправляйте до шага 3.",
                "The program computed the address from your key half — only you will control it: the seller lacks your \
                 secret half. Do not send money to the address itself before step 3.",
            ),
            (true, true) => self.t(
                "Адреса вычислены программой из вашей половины ключа — управлять ими сможете только вы: продавцу не \
                 хватает вашей секретной половины. Деньги на сами адреса не отправляйте до шага 3.",
                "The program computed the addresses from your key half — only you will control them: the seller lacks \
                 your secret half. Do not send money to the addresses before step 3.",
            ),
            (false, _) => self.t(
                "Проверка не пройдена — не оплачивайте и напишите продавцу.",
                "The check failed — do not pay, contact the seller.",
            ),
        }
        .to_string();
        self.verify = Some(out);
    }

    fn assemble(&mut self, input: &str) {
        self.last_keys = zeroize::Zeroizing::new(input.to_string());
        let ru = self.ru;
        self.wallets.clear();
        self.wallet_msg = None;
        self.show_secret = false;
        let mut out = Outcome::new();
        let mut total_codes = 0usize;
        'checks: {
            let codes = self.codes_from_input(&mut out, input, "VGKEY");
            if codes.is_empty() {
                break 'checks;
            }
            total_codes = codes.len();
            let mut halves: Vec<(usize, KeyHalf)> = Vec::new();
            for (i, code) in codes.iter().enumerate() {
                match KeyHalf::decode(code) {
                    Ok(h) => halves.push((i, h)),
                    Err(e) => out.fail(format!("{} {}: {}", self.t("Код", "Code"), i + 1, decode_error_text(e, "VGKEY", ru))),
                }
            }
            if halves.is_empty() {
                break 'checks;
            }
            out.ok(match (codes.len(), halves.len() == codes.len(), ru) {
                (1, _, true) => "Код цел: контрольная сумма сошлась, он скопирован без ошибок.".into(),
                (1, _, false) => "The code is intact: the checksum matches, it was copied without mistakes.".into(),
                (n, true, true) => format!("Кодов: {n}. Все целы: контрольные суммы сошлись, скопированы без ошибок."),
                (n, true, false) => format!("Codes: {n}. All intact: the checksums match, copied without mistakes."),
                (n, false, true) => format!("Целых кодов: {} из {n}.", halves.len()),
                (n, false, false) => format!("Intact codes: {} of {n}.", halves.len()),
            });
            let ids: Vec<String> = halves.iter().map(|(_, h)| h.order_id.clone()).collect();
            let Some(order) = self.order_of_codes(&mut out, &ids) else { break 'checks };
            let Ok(request) = Request::decode(&order.request) else {
                out.fail(self.t("Файл заявки повреждён.", "The request file is damaged.").into());
                break 'checks;
            };
            let wanted_net = order.wish.as_ref().map(|w| w.net);
            if let Some(net) = wanted_net {
                if halves.iter().all(|(_, h)| h.net == net) {
                    self.check_net(&mut out, &order, net);
                }
            }
            let verified: Vec<Proof> = order.verified_proofs.iter().filter_map(|c| Proof::decode(c).ok()).collect();
            if verified.is_empty() {
                out.warn(self.t(
                    "Шаг 2 для этой заявки не выполнялся — адреса проверяются сейчас.",
                    "Step 2 was skipped for this request — the addresses are checked now.",
                ).into());
            }

            let many = codes.len() > 1;
            let mut seen: Vec<String> = Vec::new();
            let mut seen_index: Vec<u32> = Vec::new();
            let mut good: Vec<&KeyHalf> = Vec::new();
            for (i, h) in &halves {
                let num = if many { format!("{}. ", i + 1) } else { String::new() };
                if let Some(index) = h.index {
                    if seen_index.contains(&index) {
                        out.fail(format!("{num}{} {index} {}", self.t("Для адреса №", "For address no."), self.t(
                            "прислано два кода — каждый адрес должен иметь свою половину.",
                            "two codes were sent — every address must have its own half.")));
                        continue;
                    }
                    seen_index.push(index);
                }
                if let Some(net) = wanted_net.filter(|n| *n != h.net) {
                    out.fail(format!("{num}{} {}, {} {}.", self.t("В коде сеть", "The code has network"), h.net.label(),
                        self.t("а вы заказывали", "but you ordered"), net.label()));
                    continue;
                }
                let matched = verified.iter().any(|p| *p == h.proof());
                if !verified.is_empty() && !matched {
                    out.fail(format!("{num}{}", self.t(
                        "Половина продавца НЕ совпадает ни с одной проверенной на шаге 2.",
                        "The seller's half does NOT match any half verified in step 2.",
                    )));
                    continue;
                }
                let address = match h.verify(&request) {
                    Ok(a) => a,
                    Err(_) => {
                        out.fail(format!("{num}{}", self.t(
                            "Адрес НЕ собирается из вашей половины и половины продавца.",
                            "The address does NOT come from your half and the seller's half.",
                        )));
                        continue;
                    }
                };
                if seen.contains(&address) {
                    out.fail(format!("{num}{} {address} {}", self.t("Код для адреса", "The code for address"), self.t("прислан дважды.", "was sent twice.")));
                    continue;
                }
                seen.push(address.clone());
                if Self::address_fits(&order, &address) == Some(false) {
                    out.fail(format!("{num}{} {address} {}", self.t("Адрес", "Address"), self.t("НЕ подходит под заказ.", "does NOT match your order.")));
                    continue;
                }
                if !many {
                    out.ok(format!(
                        "{} {address}{}",
                        self.t("Половины собираются в адрес", "The halves make address"),
                        if matched {
                            self.t(" — половина продавца та самая, что вы проверили на шаге 2.", " — the seller's half is exactly the one verified in step 2.")
                        } else {
                            "."
                        }
                    ));
                }
                good.push(h);
            }
            if many && !good.is_empty() {
                let made = good.len();
                let all = made == codes.len();
                let checked = !verified.is_empty();
                out.ok(match (ru, all, checked) {
                    (true, true, true) => format!("Все {made} {} продавца — те самые, что вы проверили на шаге 2; из каждой с вашей половиной собирается заказанный адрес.", ru_word(made, "половина", "половины", "половин")),
                    (true, true, false) => format!("Из всех {made} половин продавца с вашей половиной собираются заказанные адреса."),
                    (true, false, _) => format!("{made} из {} половин продавца без ошибок: из них с вашей половиной собираются заказанные адреса.", codes.len()),
                    (false, true, true) => format!("All {made} seller halves are exactly the ones verified in step 2; each one with your half makes an ordered address."),
                    (false, true, false) => format!("All {made} seller halves with your half make the ordered addresses."),
                    (false, false, _) => format!("{made} of {} seller halves are fine: with your half they make the ordered addresses.", codes.len()),
                });
            }
            if good.is_empty() {
                break 'checks;
            }

            let secrets = match vault::secrets(&order) {
                Ok(s) => s,
                Err(e) => {
                    out.fail(e.text(ru));
                    break 'checks;
                }
            };
            out.ok(self.t(
                "Ваша секретная половина прочитана из файла заявки на этом компьютере.",
                "Your secret half was read from the request file on this computer.",
            ).into());
            let mut wallets: Vec<Wallet> = Vec::new();
            let mut made_addrs: Vec<String> = Vec::new();
            for h in &good {
                let proof = verified.iter().find(|p| **p == h.proof());
                let slot = h.index.map_or(0, |i| i as usize - 1);
                let Some(a) = secrets.0.get(slot) else {
                    out.fail(SplitError::WrongIndex.text(ru).to_string());
                    continue;
                };
                let mut a_hex = match split_key::scale_secret(a, h.mult) {
                    Ok(mut s) => {
                        let text = split_key::hex(&s.secret_bytes());
                        s.non_secure_erase();
                        text
                    }
                    Err(_) => String::new(),
                };
                let (mut k, address) = match h.combine(a, &request, proof) {
                    Ok(r) => r,
                    Err(e) => {
                        out.fail(e.text(ru).to_string());
                        continue;
                    }
                };
                let mut k_hex = split_key::hex(&k.secret_bytes());
                made_addrs.push(address.clone());
                wallets.push(Wallet {
                    net: h.net,
                    address,
                    secret: split_key::export_secret(h.net, &k),
                    a_short: peek(&a_hex),
                    b_short: peek(&split_key::hex(&h.b.secret_bytes())),
                    k_short: peek(&k_hex),
                });
                k.non_secure_erase();
                k_hex.zeroize();
                a_hex.zeroize();
            }
            drop(secrets);
            if wallets.is_empty() {
                break 'checks;
            }
            out.ok(self.t(
                "Ваша половина + половина продавца = приватный ключ кошелька. Адрес каждого кошелька вычислен ещё раз — \
                 уже из готового ключа.",
                "Your half + the seller's half = the wallet private key. Every wallet address is computed once more — \
                 from the finished key.",
            ).into());
            let wanted = order.count as usize;
            let mut total = order.assembled.clone();
            for a in &made_addrs {
                if !total.contains(a) {
                    total.push(a.clone());
                }
            }
            if total.len() < wanted {
                out.warn(if ru {
                    format!("Собрано кошельков по этой заявке: {} из {wanted}. Остальные — когда продавец пришлёт их коды без ошибок.", total.len())
                } else {
                    format!("Wallets assembled for this request: {} of {wanted}. The rest — when the seller sends their codes without errors.", total.len())
                });
            }
            if let Err(e) = vault::save_assembled(&order, &made_addrs) {
                out.warn(e.text(ru));
            }
            self.reload(Some(&order.id));
            out.found = wallets.iter().map(|w| (w.net, w.address.clone())).collect();
            self.wallets = wallets;
            self.wallets_saved = false;
        }
        let n = self.wallets.len();
        out.summary = match (out.passed, n, ru) {
            (true, 1, true) => "Кошелёк собран и проверен. Ключ есть только у вас.".to_string(),
            (true, 1, false) => "The wallet is assembled and verified. Only you have the key.".to_string(),
            (true, n, true) => format!("Кошельки собраны и проверены: {n}. Ключи есть только у вас."),
            (true, n, false) => format!("Wallets assembled and verified: {n}. Only you have the keys."),
            (false, 0, true) => "Кошельки не собраны — проверка не пройдена. Напишите продавцу.".to_string(),
            (false, 0, false) => "The wallets were not assembled — a check failed. Contact the seller.".to_string(),
            (false, n, true) => format!(
                "Собрано без ошибок: {n} из {total_codes}. Этими кошельками можно пользоваться. Коды с ошибками попросите у продавца заново."
            ),
            (false, n, false) => format!(
                "Assembled without errors: {n} of {total_codes}. These wallets are ready to use. Ask the seller to resend the codes with errors."
            ),
        };
        self.assemble = Some(out);
    }

    fn save_wallets(&mut self, path: &Path) {
        let ru = self.ru;
        let Some(first) = self.wallets.first() else { return };
        let list: Vec<Value> = self
            .wallets
            .iter()
            .map(|w| {
                let mut m = serde_json::Map::new();
                m.insert(if ru { "Адрес" } else { "Address" }.into(), w.address.clone().into());
                m.insert(if ru { "Приватный ключ" } else { "Private key" }.into(), w.secret.clone().into());
                Value::Object(m)
            })
            .collect();
        let note = self.t(
            "Полные приватные ключи кошельков. Любой, у кого есть этот файл, управляет кошельками: храните его в \
             надёжном месте и никому не передавайте.",
            "Full private keys of the wallets. Anyone who has this file controls the wallets: keep it safe and \
             never share it.",
        );
        let mut text = format!(
            "{{\n  {}: {},\n  {}: {},\n  {}: {},\n  {}: {}\n}}\n",
            serde_json::to_string(if ru { "Внимание" } else { "Warning" }).unwrap_or_default(),
            serde_json::to_string(note).unwrap_or_default(),
            serde_json::to_string(if ru { "Сеть" } else { "Network" }).unwrap_or_default(),
            serde_json::to_string(first.net.label()).unwrap_or_default(),
            serde_json::to_string(if ru { "Количество" } else { "Count" }).unwrap_or_default(),
            list.len(),
            serde_json::to_string(if ru { "Кошельки" } else { "Wallets" }).unwrap_or_default(),
            serde_json::to_string_pretty(&list).unwrap_or_default().replace('\n', "\n  "),
        );
        let result = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|_| f.sync_all()));
        text.zeroize();
        for v in list {
            if let Value::Object(mut m) = v {
                for (_, val) in m.iter_mut() {
                    if let Value::String(s) = val {
                        s.zeroize();
                    }
                }
            }
        }
        if result.is_ok() {
            self.wallets_saved = true;
        }
        self.wallet_msg = Some(match result {
            Ok(()) => Msg::Ok(format!("{} {}", self.t("Сохранено:", "Saved:"), vault::shown_path(path))),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Msg::Err(self.t(
                "Файл с таким именем уже есть — выберите другое имя (существующие файлы не перезаписываются).",
                "A file with this name already exists — choose another name (existing files are never overwritten).",
            ).into()),
            Err(e) => Msg::Err(format!("{} {e}", self.t("Не удалось сохранить:", "Could not save:"))),
        });
    }

    fn step_subs(&self) -> [String; 3] {
        let done = self.progress();
        let count = self.order().map_or(1, |o| o.count);
        let (verified, assembled) = self.order().map_or((0, 0), |o| (o.verified_proofs.len(), o.assembled.len()));
        let sub1 = if done[0] {
            self.order().map(|o| format!("#{} · {}", o.id, date(o.created))).unwrap_or_default()
        } else {
            self.t("Сеть и красивый адрес", "Network and a nice address").to_string()
        };
        let sub2 = match (done[1], verified > 0, self.ru) {
            (true, _, true) if count > 1 => format!("Все {count} {} настоящие", ru_word(count as usize, "адрес", "адреса", "адресов")),
            (true, _, false) if count > 1 => format!("All {count} addresses genuine"),
            (true, _, _) => self.t("Адрес настоящий", "Address is genuine").to_string(),
            _ => self.t("До оплаты · VGPROOF", "Before paying · VGPROOF").to_string(),
        };
        let sub3 = match (done[2], assembled > 0, self.ru) {
            (true, _, true) => format!("Собрано: {assembled}"),
            (true, _, false) => format!("Assembled: {assembled}"),
            (false, true, true) => format!("Собрано {assembled} из {count}"),
            (false, true, false) => format!("Assembled {assembled} of {count}"),
            _ => self.t("После оплаты · VGKEY", "After paying · VGKEY").to_string(),
        };
        [sub1, sub2, sub3]
    }

    pub fn view(&self) -> Value {
        let ru = self.ru;
        let wish = self.order().and_then(|o| o.wish.clone());
        let order = self.order().map(|o| {
            let parts = Request::decode(&o.request).ok().map(|r| {
                vec![
                    json!([if r.legacy { "VGREQ1" } else { "VGREQ2" }, self.t("метка: это код заявки", "label: this is a request code")]),
                    json!([short(&split_key::hex(&r.points[0].serialize())),
                        self.t("ваша публичная половина — «отпечаток», секрета в нём нет", "your public half — a “fingerprint”, no secret in it")]),
                    json!([o.request.rsplit(':').next().unwrap_or(""),
                        self.t("контрольная сумма — защита от ошибок при копировании", "checksum — protection against copy mistakes")]),
                    json!([o.id, self.t("номер заявки — по нему коды продавца связываются с этой заявкой",
                        "request number — links the seller's codes to this request")]),
                ]
            });
            let (net, start, end) = match &o.wish {
                Some(w) => {
                    let body = wanted_body(w.net, &w.start);
                    let start = if body.is_empty() { "—".to_string() } else { format!("{}{body}", fixed_start(w.net)) };
                    let end = if w.end.trim().is_empty() { "—".to_string() } else { w.end.trim().to_string() };
                    (w.net.label().to_string(), start, end)
                }
                None => ("—".into(), "—".into(), "—".into()),
            };
            json!({
                "id": o.id,
                "date": date(o.created),
                "net": net,
                "start": start,
                "end": end,
                "count": o.count,
                "path": vault::shown_path(&o.path),
                "code": o.request,
                "message": seller_message(o, ru),
                "parts": parts,
                "secret_hint": self.hints.borrow_mut().entry(o.path.clone())
                    .or_insert_with(|| vault::secret_hint(o)).clone(),
            })
        });
        let wallets: Vec<Value> = self
            .wallets
            .iter()
            .map(|w| {
                let mut v = address_view(wish.as_ref(), &w.address);
                v["a"] = json!(w.a_short);
                v["b"] = json!(w.b_short);
                v["k"] = json!(w.k_short);
                if self.show_secret {
                    v["key"] = json!(w.secret);
                }
                v
            })
            .collect();
        json!({
            "ru": ru,
            "step": self.step as u8,
            "done": self.progress(),
            "current": self.next_step() as u8,
            "subs": self.step_subs(),
            "orders": self.orders.iter().map(|o| json!({
                "id": o.id,
                "net": o.wish.as_ref().map_or("—", |w| w.net.label()),
                "count": o.count,
                "date": date(o.created),
                "verified": o.verified_proofs.len() >= o.count as usize,
                "assembled": o.assembled.len() >= o.count as usize,
            })).collect::<Vec<_>>(),
            "missing": self.missing.iter().map(|p| json!({
                "name": p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
                "path": vault::shown_path(p),
                "exists": p.exists(),
            })).collect::<Vec<_>>(),
            "selected": if self.new_mode { -1 } else { self.selected as i64 },
            "new_mode": self.new_mode,
            "form_epoch": self.form_epoch,
            "busy": self.pick.is_some(),
            "saving_request": matches!(self.pick, Some(Pick::SaveRequest { .. })),
            "nets": Network::ALL.iter().map(|n| json!({
                "tag": n.tag(),
                "label": n.label(),
                "fixed": fixed_start(*n),
                "len": typical_len(*n),
                "evm": n.is_evm(),
                "alpha": Charset::of(*n).alphabet(),
            })).collect::<Vec<_>>(),
            "order": order,
            "create_msg": msg_view(&self.create_msg),
            "copy_msg": msg_view(&self.copy_msg),
            "open_msg": msg_view(&self.open_msg),
            "verify": self.verify.as_ref().map(|o| o.view(wish.as_ref())),
            "assemble": self.assemble.as_ref().map(|o| o.view(wish.as_ref())),
            "wallets": wallets,
            "wallets_net": self.wallets.first().map(|w| w.net.label()),
            "show_keys": self.show_secret,
            "closing": self.closing,
            "wallets_saved": self.wallets_saved,
            "wallet_msg": msg_view(&self.wallet_msg),
        })
    }

    pub fn unsaved_keys(&self) -> bool {
        !self.wallets.is_empty() && !self.wallets_saved
    }

    pub fn ask_close(&mut self) {
        self.closing = true;
    }

    pub fn forget_secrets(&mut self) {
        self.wallets.clear();
        self.last_keys.zeroize();
    }
}

pub fn seller_message(order: &vault::Order, ru: bool) -> String {
    let mut m = String::new();
    m.push_str(if ru { "Заказ vanity-кошелька (разделённый ключ)\n" } else { "Vanity wallet order (split key)\n" });
    if let Some(w) = &order.wish {
        let fixed = fixed_start(w.net);
        let start = wanted_body(w.net, &w.start);
        let dash = "—";
        m.push_str(&format!("{} {}\n", if ru { "Сеть:" } else { "Network:" }, w.net.label()));
        m.push_str(&format!(
            "{} {}\n",
            if ru { "Начало адреса:" } else { "Address start:" },
            if start.is_empty() { dash.to_string() } else { format!("{fixed}{start}") }
        ));
        m.push_str(&format!(
            "{} {}\n",
            if ru { "Конец адреса:" } else { "Address end:" },
            if w.end.trim().is_empty() { dash } else { w.end.trim() }
        ));
    }
    m.push_str(&format!("{} {}\n", if ru { "Количество адресов:" } else { "Number of addresses:" }, order.count));
    m.push_str(&format!("{} {}", if ru { "Код заявки:" } else { "Request code:" }, order.request));
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn half(order: &vault::Order, index: u32, seed: u8) -> (KeyHalf, String) {
        let request = Request::decode(&order.request).unwrap();
        let b = secp256k1::SecretKey::from_slice(&[seed; 32]).unwrap();
        let proof = Proof::signed(&request, index, Network::Tron, &b).unwrap().encode();
        (KeyHalf { order_id: order.id.clone(), index: Some(index), net: Network::Tron, claimed: None, b, mult: 0 }, proof)
    }

    #[test]
    fn language_switch_keeps_results() {
        let dir = std::env::temp_dir().join(format!("vwb_test_lang_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let wish = Wish { net: Network::Tron, start: String::new(), end: String::new() };
        let order = vault::Draft::new(1).save(&dir.join("lang.json"), &wish, true).unwrap();
        let mut b = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut b);
        b[0] = 1;
        let (h, proof) = half(&order, 1, 1);
        let h = KeyHalf { b: secp256k1::SecretKey::from_slice(&b).unwrap(), ..h };
        let proof = Proof::signed(&Request::decode(&order.request).unwrap(), 1, Network::Tron, &h.b).map(|p| p.encode()).unwrap_or(proof);
        let mut app = BuyerApp::new();
        app.act(&json!({"a": "verify", "text": proof}));
        assert!(app.verify.as_ref().unwrap().passed);
        app.act(&json!({"a": "lang", "ru": false}));
        let v = app.verify.as_ref().unwrap();
        assert!(v.passed && v.summary.starts_with("The program"));
        app.act(&json!({"a": "assemble", "text": h.encode()}));
        assert_eq!(app.wallets.len(), 1);
        app.act(&json!({"a": "toggle_keys"}));
        app.act(&json!({"a": "lang", "ru": true}));
        assert!(app.assemble.as_ref().unwrap().passed);
        assert_eq!(app.wallets.len(), 1);
        assert!(app.show_secret);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn many_addresses_give_one_summary_line() {
        let dir = std::env::temp_dir().join(format!("vwb_test_many_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let wish = Wish { net: Network::Tron, start: String::new(), end: String::new() };
        let order = vault::Draft::new(3).save(&dir.join("many.json"), &wish, true).unwrap();
        let pairs: Vec<(KeyHalf, String)> = (1..=3u32).map(|n| half(&order, n, n as u8 + 7)).collect();
        let proofs = pairs.iter().map(|p| p.1.clone()).collect::<Vec<_>>().join("\n");
        let keys = |list: &[(KeyHalf, String)]| list.iter().map(|p| p.0.encode()).collect::<Vec<_>>().join("\n");
        let mut app = BuyerApp::new();
        app.act(&json!({"a": "verify", "text": proofs}));
        let v = app.verify.as_ref().unwrap();
        assert!(v.passed, "{:?}", v.checks.iter().map(|c| c.text.clone()).collect::<Vec<_>>());
        assert_eq!(v.checks.iter().filter(|c| c.text.starts_with("Все 3")).count(), 1);
        assert!(v.checks.iter().all(|c| !c.text.contains("получается адрес")));
        app.act(&json!({"a": "assemble", "text": keys(&pairs)}));
        let a = app.assemble.as_ref().unwrap();
        assert!(a.passed);
        assert_eq!(a.checks.iter().filter(|c| c.text.starts_with("Все 3")).count(), 1);
        let view = app.view();
        let w = &view["wallets"][0];
        assert_eq!(w["k"].as_str().unwrap().chars().count(), 13);
        assert!(w.get("key").is_none());
        assert!(app.unsaved_keys());
        let firsts: Vec<String> = app.wallets.iter().map(|w| w.a_short.clone()).collect();
        assert!(firsts[0] != firsts[1] && firsts[1] != firsts[2]);

        let mut codes: Vec<String> = pairs.iter().map(|p| p.0.encode()).collect();
        let last = codes[2].pop().unwrap();
        codes[2].push(if last == '0' { '1' } else { '0' });
        app.act(&json!({"a": "assemble", "text": codes.join("\n")}));
        let a = app.assemble.as_ref().unwrap();
        assert!(!a.passed);
        assert_eq!(app.wallets.len(), 2);
        assert!(a.summary.starts_with("Собрано без ошибок: 2 из 3"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn one_half_for_two_addresses_is_refused() {
        let dir = std::env::temp_dir().join(format!("vwb_test_dup_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let wish = Wish { net: Network::Tron, start: String::new(), end: String::new() };
        let order = vault::Draft::new(2).save(&dir.join("dup.json"), &wish, true).unwrap();
        let (_, p1) = half(&order, 1, 9);
        let (_, p1b) = half(&order, 1, 10);
        let mut app = BuyerApp::new();
        app.act(&json!({"a": "verify", "text": format!("{p1}\n{p1b}")}));
        let v = app.verify.as_ref().unwrap();
        assert!(!v.passed);
        assert!(v.checks.iter().any(|c| c.text.contains("каждый адрес должен иметь свою половину")));

        let request = Request::decode(&order.request).unwrap();
        let b = secp256k1::SecretKey::from_slice(&[11; 32]).unwrap();
        let unsigned = Proof { signature: None, index: None, ..Proof::signed(&request, 1, Network::Tron, &b).unwrap() };
        app.act(&json!({"a": "verify", "text": unsigned.encode()}));
        assert!(!app.verify.as_ref().unwrap().passed);
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn dates() {
        assert_eq!(date(0), "01.01.1970");
        assert_eq!(date(951_782_400), "29.02.2000");
        assert_eq!(date(1_793_313_266), "29.10.2026");
    }

    #[test]
    fn wish_matching() {
        let w = |net, start: &str, end: &str| Wish { net, start: start.into(), end: end.into() };
        let tron = "TH3PWxLoDjwGSiceCVmPGGgiCnbZqbpPet";
        assert!(wish_matches(&w(Network::Tron, "H3PW", ""), tron));
        assert!(wish_matches(&w(Network::Tron, "TH3PW", ""), tron));
        assert!(wish_matches(&w(Network::Tron, "", "Pet"), tron));
        assert!(!wish_matches(&w(Network::Tron, "H3PX", ""), tron));
        let eth = "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf";
        assert!(wish_matches(&w(Network::Eth, "7E5F", ""), eth));
        assert!(wish_matches(&w(Network::Eth, "0x7E5F", "Bdf"), eth));
        assert!(!wish_matches(&w(Network::Eth, "7e5f", ""), eth));
        let bc1 = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
        assert!(wish_matches(&w(Network::BtcP2wpkh, "w508", ""), bc1));
        assert!(wish_matches(&w(Network::BtcP2wpkh, "bc1qw508", ""), bc1));
        assert!(charset_ok(Network::Tron, "Pet") && !charset_ok(Network::Tron, "P0t"));
        assert!(charset_ok(Network::Eth, "cafe") && !charset_ok(Network::Eth, "cafg"));
        assert!(!charset_ok(Network::BtcP2tr, "b"));
    }

    #[test]
    fn wish_highlight() {
        let w = |start: &str, end: &str| Wish { net: Network::Tron, start: start.into(), end: end.into() };
        let a = "TFFFF2PrLxAvQruE3uHzcjS1QVPnTxFXEF";
        assert_eq!(wish_marks(Some(&w("FFFF", "F")), a), (5, 1));
        assert_eq!(wish_marks(Some(&w("TFFFF", "")), a), (5, 0));
        assert_eq!(wish_marks(Some(&w("", "XEF")), a), (0, 3));
        assert_eq!(wish_marks(Some(&w("Pet", "")), a), (0, 0));
        assert_eq!(wish_marks(None, a), (0, 0));
        let v = address_view(Some(&w("FFFF", "F")), a);
        assert_eq!((v["head"].as_str(), v["tail"].as_str()), (Some("TFFFF"), Some("F")));
    }

    #[test]
    fn typical_lengths_and_wishes() {
        use secp256k1::{PublicKey, Secp256k1, SecretKey};
        let secp = Secp256k1::new();
        for n in 1..40u8 {
            let pk = PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&[n; 32]).unwrap());
            for net in Network::ALL {
                let a = split_key::address(net, &pk);
                assert!(charset_ok(net, wanted_body(net, &a)), "{net:?}: {a}");
                assert!(a.len() <= typical_len(net) && a.len() + 2 >= typical_len(net), "{net:?}: {a}");
                let w = |start: &str, end: &str| Wish { net, start: start.into(), end: end.into() };
                assert!(wish_matches(&w(&a[..fixed_start(net).len() + 2], &a[a.len() - 2..]), &a));
            }
        }
        let w = |net, start: &str| Wish { net, start: start.into(), end: String::new() };
        let bsc = "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf";
        assert!(wish_matches(&w(Network::Bsc, "0X7E5F"), bsc) && !wish_matches(&w(Network::Bsc, "7e5f"), bsc));
    }

    #[test]
    fn wish_problems_are_explained() {
        assert!(wish_problem(Network::Tron, "Pet", "", true).is_none());
        assert!(wish_problem(Network::Tron, "", "", true).is_some());
        let e = wish_problem(Network::Tron, "P0t", "l", true).unwrap();
        assert!(e.contains("«0»") && e.contains("«l»") && e.contains("0  O  I  l"), "{e}");
        let e = wish_problem(Network::Eth, "cafe", "xyz", false).unwrap();
        assert!(e.contains("«x»") && e.contains("g–z"), "{e}");
        let e = wish_problem(Network::BtcP2wpkh, "Bad", "", true).unwrap();
        assert!(e.contains("«B»") && e.contains("1  b  i  o"), "{e}");
        let e = wish_problem(Network::Tron, "1abc", "", true).unwrap();
        assert!(e.contains("9 A B C"), "{e}");
        assert!(wish_problem(Network::Tron, "a b", "", false).unwrap().contains("space"));
        assert!(wish_problem(Network::Tron, &"A".repeat(21), "", true).is_some());
    }

    fn cyrillic(v: &Value) -> Vec<String> {
        let mut out = Vec::new();
        match v {
            Value::String(s) if s.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c)) => out.push(s.clone()),
            Value::Array(a) => a.iter().for_each(|x| out.extend(cyrillic(x))),
            Value::Object(o) => o.iter().filter(|(k, _)| *k != "path" && *k != "nets").for_each(|(_, x)| out.extend(cyrillic(x))),
            _ => {}
        }
        out
    }

    #[test]
    fn english_mode_has_no_russian_anywhere() {
        let dir = std::env::temp_dir().join(format!("vwb_test_en_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let wish = Wish { net: Network::Tron, start: String::new(), end: String::new() };
        let order = vault::Draft::new(3).save(&dir.join("en.json"), &wish, false).unwrap();
        let pairs: Vec<(KeyHalf, String)> = (1..=3u32).map(|n| half(&order, n, n as u8 + 40)).collect();
        let proofs = pairs.iter().map(|p| p.1.clone()).collect::<Vec<_>>().join("\n");
        let keys = pairs.iter().map(|p| p.0.encode()).collect::<Vec<_>>().join("\n");
        let mut app = BuyerApp::new();
        app.act(&json!({"a": "lang", "ru": false}));
        let pos = app.orders.iter().position(|o| o.id == order.id).unwrap();
        app.act(&json!({"a": "select", "i": pos}));
        let mut views = vec![app.view()];
        app.act(&json!({"a": "copy_message"}));
        views.push(app.view());
        for bad in ["garbage", "VGPROOF3:00000000:1:TRON:00:00:00000000", &pairs[0].1, &format!("{}\n{}", pairs[0].1, pairs[0].1)] {
            app.act(&json!({"a": "verify", "text": bad}));
            assert!(!app.verify.as_ref().unwrap().passed);
            views.push(app.view());
        }
        app.act(&json!({"a": "verify", "text": proofs}));
        assert!(app.verify.as_ref().unwrap().passed);
        views.push(app.view());
        let other = KeyHalf { b: secp256k1::SecretKey::from_slice(&[99; 32]).unwrap(), ..pairs[0].0.clone() };
        for bad in ["garbage".to_string(), other.encode(), format!("{}\n{}", pairs[1].0.encode(), pairs[1].0.encode())] {
            app.act(&json!({"a": "assemble", "text": bad}));
            assert!(!app.assemble.as_ref().unwrap().passed);
            views.push(app.view());
        }
        app.act(&json!({"a": "assemble", "text": keys}));
        assert!(app.assemble.as_ref().unwrap().passed);
        app.act(&json!({"a": "toggle_keys"}));
        app.act(&json!({"a": "copy_address", "i": 0}));
        views.push(app.view());
        for p in [wish_problem(Network::Tron, "P0t", "l", false), wish_problem(Network::Tron, "1abc", "", false), wish_problem(Network::Eth, "", "", false)] {
            views.push(json!(p));
        }
        for v in &views {
            let found = cyrillic(v);
            assert!(found.is_empty(), "{found:?}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn russian_counts_agree_with_numbers() {
        let words = |n| ru_word(n, "адрес", "адреса", "адресов");
        for (n, w) in [(1, "адрес"), (2, "адреса"), (4, "адреса"), (5, "адресов"), (11, "адресов"), (12, "адресов"), (21, "адрес"), (22, "адреса"), (100, "адресов")] {
            assert_eq!(words(n), w, "{n}");
        }
    }

    #[test]
    fn seller_half_cannot_be_faked_or_skipped() {
        let dir = std::env::temp_dir().join(format!("vwb_test_fake_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let wish = Wish { net: Network::Tron, start: String::new(), end: String::new() };
        let order = vault::Draft::new(2).save(&dir.join("fake.json"), &wish, true).unwrap();
        let request = Request::decode(&order.request).unwrap();
        let (h1, p1) = half(&order, 1, 51);
        let (h2, p2) = half(&order, 2, 52);
        let mut app = BuyerApp::new();
        app.act(&json!({"a": "verify", "text": format!("{p1}\n{p2}")}));
        assert!(app.verify.as_ref().unwrap().passed);

        let x = secp256k1::SecretKey::from_slice(&[77; 32]).unwrap();
        let secp = secp256k1::Secp256k1::new();
        let neg_a = request.points[0].negate(&secp);
        let rogue_b = secp256k1::PublicKey::from_secret_key(&secp, &x).combine(&neg_a).unwrap();
        let mut rogue = Proof::decode(&p1).unwrap();
        rogue.b_pub = rogue_b;
        app.act(&json!({"a": "verify", "text": format!("{}\n{p2}", rogue.encode())}));
        let v = app.verify.as_ref().unwrap();
        assert!(!v.passed);
        assert!(v.checks.iter().any(|c| c.text.contains("подпись не сошлась")), "{:?}", v.checks.iter().map(|c| &c.text).collect::<Vec<_>>());

        let swapped = Proof::decode(&p2).map(|p| Proof { index: Some(1), ..p }).unwrap();
        app.act(&json!({"a": "verify", "text": format!("{}\n{p2}", swapped.encode())}));
        assert!(!app.verify.as_ref().unwrap().passed);

        app.act(&json!({"a": "verify", "text": format!("{p1}\n{p2}")}));
        assert!(app.verify.as_ref().unwrap().passed);
        let wrong = KeyHalf { b: secp256k1::SecretKey::from_slice(&[53; 32]).unwrap(), ..h1.clone() };
        app.act(&json!({"a": "assemble", "text": format!("{}\n{}", wrong.encode(), h2.encode())}));
        let a = app.assemble.as_ref().unwrap();
        assert!(!a.passed);
        assert_eq!(app.wallets.len(), 1);
        let crossed = KeyHalf { index: Some(2), ..h1.clone() };
        app.act(&json!({"a": "assemble", "text": crossed.encode()}));
        assert!(!app.assemble.as_ref().unwrap().passed);
        assert!(app.wallets.is_empty());
        app.act(&json!({"a": "assemble", "text": p1}));
        assert!(!app.assemble.as_ref().unwrap().passed && app.wallets.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn interop_dir() -> PathBuf {
        PathBuf::from(std::env::var("VWB_INTEROP").expect("VWB_INTEROP"))
    }

    pub fn interop_end(net: Network) -> &'static str {
        match net.encoding() {
            Encoding::Evm => "77",
            Encoding::SegwitV0 { .. } | Encoding::Taproot | Encoding::Bech32 { .. } => "qq",
            _ => "zz",
        }
    }

    #[test]
    #[ignore = "interop with the seller program"]
    fn interop_make_requests() {
        let dir = interop_dir();
        let mut out = String::new();
        for net in Network::ALL {
            let wish = Wish { net, start: String::new(), end: interop_end(net).into() };
            let order = vault::Draft::new(3).save(&dir.join(format!("{}.json", net.tag())), &wish, false).unwrap();
            out.push_str(&format!("{}|{}|{}\n", net.tag(), interop_end(net), seller_message(&order, false).replace('\n', "\\n")));
        }
        std::fs::write(dir.join("requests.txt"), out).unwrap();
    }

    #[test]
    #[ignore = "interop with the seller program"]
    fn interop_assemble() {
        let dir = interop_dir();
        let text = std::fs::read_to_string(dir.join("answers.txt")).unwrap();
        let mut out = String::new();
        for line in text.lines().filter(|l| !l.is_empty()) {
            let parts: Vec<&str> = line.split('|').collect();
            let (tag, proofs, keys) = (parts[0], parts[1].replace(' ', "\n"), parts[2].replace(' ', "\n"));
            let order = vault::open(&dir.join(format!("{tag}.json"))).unwrap();
            let mut app = BuyerApp::new();
            let pos = app.orders.iter().position(|o| o.id == order.id).unwrap();
            app.act(&json!({"a": "select", "i": pos}));
            app.act(&json!({"a": "verify", "text": proofs}));
            let v = app.verify.as_ref().unwrap();
            assert!(v.passed, "{tag}: {:?}", v.checks.iter().map(|c| &c.text).collect::<Vec<_>>());
            assert_eq!(v.found.len(), 3, "{tag}");
            app.act(&json!({"a": "assemble", "text": keys}));
            let a = app.assemble.as_ref().unwrap();
            assert!(a.passed, "{tag}: {:?}", a.checks.iter().map(|c| &c.text).collect::<Vec<_>>());
            app.act(&json!({"a": "toggle_keys"}));
            let view = app.view();
            let wallets = view["wallets"].as_array().unwrap();
            assert_eq!(wallets.len(), 3, "{tag}");
            for w in wallets {
                out.push_str(&format!("{tag}|{}|{}\n", w["full"].as_str().unwrap(), w["key"].as_str().unwrap()));
            }
        }
        std::fs::write(dir.join("wallets.txt"), out).unwrap();
    }

    #[test]
    fn keys_are_hidden_from_the_view_until_shown() {
        let mut app = BuyerApp::new();
        app.wallets.push(Wallet {
            net: Network::Tron,
            address: "TAddr".into(),
            secret: "SECRETKEY".into(),
            a_short: "a".into(),
            b_short: "b".into(),
            k_short: "k".into(),
        });
        assert!(!app.view().to_string().contains("SECRETKEY"));
        app.act(&json!({"a": "toggle_keys"}));
        assert!(app.view().to_string().contains("SECRETKEY"));
        app.act(&json!({"a": "toggle_keys"}));
        assert!(!app.view().to_string().contains("SECRETKEY"));
    }
}
