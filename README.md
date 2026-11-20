# Vanity Wallets Buyer

Desktop client for ordering vanity crypto addresses with a **split key**: the seller searches for the address,
but only you ever hold the full private key.

**[English](#english)** · **[Русский](#русский)**

---

## English

### How it works

The wallet key is the sum of two halves: `k = a + b (mod n)`.

- `a` — your half. Generated on your computer and stored only in your request file.
- `b` — the seller's half. The seller searches for `b` such that the address of `A + B` matches your pattern,
  knowing only your public point `A = a·G`.
- The seller never learns `a`, so they cannot compute `k`. You get `b` after payment and the program adds the
  halves locally.

### Workflow

| Step | You do | The program does |
|---|---|---|
| 1. Request | Choose network, address start/end, quantity (1–100); save the request file; paste the generated message into the order on the site | Generates `a`, writes the request file, builds the `VGREQ1` code |
| 2. Verify — before paying | Paste the seller's `VGPROOF` codes (all at once, any formatting) | Derives every address from `A + B` itself, checks pattern, quantity, duplicates and network |
| 3. Wallets — after paying | Paste the seller's `VGKEY` codes | Checks each half against step 2, assembles `k`, re-derives every address from the final key; wallets can be shown, copied and saved to one file |

If some `VGKEY` codes are broken, the valid ones are still assembled and usable; the broken ones are listed so
you can ask the seller to resend them.

### Codes

| Code | Sent by | Contains |
|---|---|---|
| `VGREQ1:<A>:<checksum>` | buyer → seller | Your public point `A` |
| `VGPROOF2:<id>:<net>:<B>:<checksum>` | seller → buyer, before payment | Seller's public point `B`. No address — the program computes it |
| `VGKEY2:<id>:<net>:<b>:<checksum>` | seller → buyer, after payment | Seller's half `b` |

Legacy `VGPROOF1` / `VGKEY1` codes (with an address inside) are accepted; the claimed address is checked
against the derived one.

### Security

- `a` comes from the Windows CSPRNG (`BCryptGenRandom` via `rand::rngs::OsRng`).
- The request file holds `a` in plain text. Keep it private and do not delete it until the wallets are
  assembled. Files are never overwritten; cancelling an order overwrites the file with zeros and deletes it.
- Full keys appear on screen only after **Show keys**. Copied keys are excluded from clipboard history and
  cloud sync and are wiped from the clipboard on exit. Secrets in the Rust code are zeroized after use.
- No network code. The UI is HTML embedded in the exe and served by the program itself; its Content Security
  Policy blocks all requests, navigation and new windows. The Microsoft WebView2 runtime that renders the
  window may contact Microsoft services on its own; the program works fully offline, so you may disconnect
  before running it.
- One instance at a time. Closing with assembled but unsaved wallets asks for confirmation.
- Crash dumps are disabled so that no memory image with keys is written to disk.

### Data on disk

| Location | Contents |
|---|---|
| Request file (where you save it) | Request number, network, pattern, quantity, `VGREQ1` code, your half `a`, verified codes, assembled addresses |
| Wallets file (where you save it) | Addresses and full private keys |
| `%LOCALAPPDATA%\VanityBuyer` | List of request file paths and the WebView2 cache. No secrets |

### Networks

TRON · Ethereum and EVM (BNB Smart Chain, Polygon, Arbitrum, Optimism, Base, Avalanche C-Chain, Linea,
zkSync Era, HyperEVM) · Bitcoin (1…, 3…, bc1q…, bc1p…) · Litecoin (L…, M…, ltc1q…) · Dogecoin · Dash ·
XRP Ledger · Cosmos Hub, Osmosis, Celestia, dYdX, Akash, Juno, Stargaze, Noble.

Keys are exported as WIF for Bitcoin-family networks and as hex otherwise. XRP keys are raw secp256k1 hex,
not `s…` seeds. Import into a wallet that supports the address type you ordered.

### Requirements

Windows 10 or 11, 64-bit, with Microsoft Edge WebView2 Runtime (built into Windows 11, installed with Edge on
Windows 10). Without it the program shows a message and exits.

### Build

1. Install Rust from https://www.rust-lang.org/tools/install, with Visual Studio Build Tools
   ("Desktop development with C++").
2. In the source folder:

```
cargo build --release --locked
cargo test --release --locked
```

The executable is `target\release\VanityWalletsBuyer.exe`. `--locked` builds exactly the dependency versions
pinned in `Cargo.lock`; the C runtime is linked statically (`.cargo\config.toml`).

### Source layout

| Path | Purpose |
|---|---|
| `src/split_key.rs` | Codes, key addition, address derivation for all networks |
| `src/logic.rs` | Application logic: requests, verification, assembly, UI state |
| `src/vault.rs` | Request files and the request list |
| `src/patterns.rs` | Allowed characters and reachable address prefixes |
| `src/dialogs.rs` | Native Windows open/save dialogs |
| `src/main.rs` | Window, WebView2 setup, IPC, clipboard |
| `ui/` | Interface (HTML/CSS/JS) and fonts |
| `build.rs` | Embeds icon, version info and manifest |

### Dependencies

`secp256k1`, `sha2`, `ripemd`, `tiny-keccak`, `bs58`, `bech32` (cryptography and encodings) · `rand` (OS RNG) ·
`zeroize` · `serde`, `serde_json` · `tao`, `wry` (window and WebView2, Tauri project) · `arboard` (clipboard) ·
`windows` (Microsoft Win32 bindings). All from crates.io, versions pinned in `Cargo.lock`.

Fonts: IBM Plex Sans, JetBrains Mono, Unbounded — SIL Open Font License, texts in `ui/fonts/`.

---

## Русский

### Принцип работы

Ключ кошелька — сумма двух половин: `k = a + b (mod n)`.

- `a` — ваша половина. Создаётся на вашем компьютере и хранится только в файле заявки.
- `b` — половина продавца. Продавец подбирает `b` так, чтобы адрес `A + B` подходил под ваш шаблон, зная только
  вашу открытую точку `A = a·G`.
- Продавец не знает `a` и не может вычислить `k`. Вы получаете `b` после оплаты, программа складывает половины
  у вас.

### Порядок работы

| Шаг | Вы | Программа |
|---|---|---|
| 1. Заявка | Выбираете сеть, начало/конец адреса, количество (1–100); сохраняете файл заявки; вставляете готовое сообщение в заказ на сайте | Создаёт `a`, записывает файл заявки, формирует код `VGREQ1` |
| 2. Проверка — до оплаты | Вставляете коды продавца `VGPROOF` (все сразу, в любом виде) | Сама вычисляет каждый адрес из `A + B`, проверяет шаблон, количество, повторы и сеть |
| 3. Кошельки — после оплаты | Вставляете коды продавца `VGKEY` | Сверяет каждую половину с шагом 2, собирает `k`, заново вычисляет адрес из готового ключа; кошельки можно показать, скопировать и сохранить в один файл |

Если часть кодов `VGKEY` повреждена, исправные всё равно собираются и готовы к использованию; повреждённые
перечислены, чтобы запросить их у продавца заново.

### Коды

| Код | Кто отправляет | Содержимое |
|---|---|---|
| `VGREQ1:<A>:<сумма>` | покупатель → продавцу | Ваша открытая точка `A` |
| `VGPROOF2:<id>:<сеть>:<B>:<сумма>` | продавец → покупателю, до оплаты | Открытая точка продавца `B`. Адреса нет — его вычисляет программа |
| `VGKEY2:<id>:<сеть>:<b>:<сумма>` | продавец → покупателю, после оплаты | Половина продавца `b` |

Старые коды `VGPROOF1` / `VGKEY1` (с адресом внутри) принимаются; указанный в них адрес сверяется с вычисленным.

### Безопасность

- `a` берётся из криптографического генератора Windows (`BCryptGenRandom` через `rand::rngs::OsRng`).
- Файл заявки хранит `a` открытым текстом. Не передавайте его и не удаляйте до сборки кошельков. Файлы никогда
  не перезаписываются; при отмене заказа файл затирается нулями и удаляется.
- Полные ключи показываются только по кнопке **Показать ключи**. Скопированный ключ исключается из журнала буфера
  обмена и облачной синхронизации и стирается из буфера при выходе. Секреты в Rust-коде затираются после
  использования.
- Сетевого кода нет. Интерфейс — HTML внутри exe, его отдаёт сама программа; политика безопасности страницы
  запрещает любые запросы, переходы и новые окна. Движок Microsoft WebView2, который рисует окно, может сам
  обращаться к сервисам Microsoft; программа полностью работает без интернета, его можно отключить перед запуском.
- Запускается только одна копия. Закрытие с собранными, но не сохранёнными кошельками требует подтверждения.
- Дампы памяти при сбое отключены, чтобы образ памяти с ключами не попал на диск.

### Данные на диске

| Где | Что |
|---|---|
| Файл заявки (куда сохраните) | Номер заявки, сеть, шаблон, количество, код `VGREQ1`, ваша половина `a`, проверенные коды, собранные адреса |
| Файл кошельков (куда сохраните) | Адреса и полные приватные ключи |
| `%LOCALAPPDATA%\VanityBuyer` | Список путей к файлам заявок и кэш WebView2. Секретов нет |

### Сети

TRON · Ethereum и EVM (BNB Smart Chain, Polygon, Arbitrum, Optimism, Base, Avalanche C-Chain, Linea,
zkSync Era, HyperEVM) · Bitcoin (1…, 3…, bc1q…, bc1p…) · Litecoin (L…, M…, ltc1q…) · Dogecoin · Dash ·
XRP Ledger · Cosmos Hub, Osmosis, Celestia, dYdX, Akash, Juno, Stargaze, Noble.

Ключи выдаются в формате WIF для сетей семейства Bitcoin и в hex для остальных. Ключ XRP — hex-ключ secp256k1,
а не seed вида `s…`. Импортируйте ключ в кошелёк, который поддерживает заказанный тип адреса.

### Требования

Windows 10 или 11, 64-бит, с Microsoft Edge WebView2 Runtime (встроен в Windows 11, ставится вместе с Edge в
Windows 10). Без него программа сообщит об этом и закроется.

### Сборка

1. Установите Rust с https://www.rust-lang.org/tools/install вместе с Visual Studio Build Tools
   («Desktop development with C++»).
2. В папке с исходниками:

```
cargo build --release --locked
cargo test --release --locked
```

Готовый файл: `target\release\VanityWalletsBuyer.exe`. `--locked` собирает ровно те версии зависимостей, что
зафиксированы в `Cargo.lock`; C-runtime встраивается в exe (`.cargo\config.toml`).

### Структура исходников

| Путь | Назначение |
|---|---|
| `src/split_key.rs` | Коды, сложение ключей, вычисление адресов всех сетей |
| `src/logic.rs` | Логика: заявки, проверка, сборка, состояние интерфейса |
| `src/vault.rs` | Файлы заявок и список заявок |
| `src/patterns.rs` | Допустимые символы и достижимые начала адресов |
| `src/dialogs.rs` | Стандартные окна Windows «Открыть» / «Сохранить» |
| `src/main.rs` | Окно, настройка WebView2, обмен с интерфейсом, буфер обмена |
| `ui/` | Интерфейс (HTML/CSS/JS) и шрифты |
| `build.rs` | Иконка, сведения о версии и манифест exe |

### Зависимости

`secp256k1`, `sha2`, `ripemd`, `tiny-keccak`, `bs58`, `bech32` (криптография и кодировки) · `rand` (генератор ОС) ·
`zeroize` · `serde`, `serde_json` · `tao`, `wry` (окно и WebView2, проект Tauri) · `arboard` (буфер обмена) ·
`windows` (привязки Win32 от Microsoft). Все с crates.io, версии зафиксированы в `Cargo.lock`.

Шрифты: IBM Plex Sans, JetBrains Mono, Unbounded — SIL Open Font License, тексты лицензий в `ui/fonts/`.
