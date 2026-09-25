# Pylotaxis

Portale desktop che cataloga, protegge e avvia da un unico punto le app che sviluppo, locali e web.
Local-first: nessun server, nessuna telemetria, funzionamento interamente offline.

Il metodo di lavoro, la Scheda progetto e la roadmap sono nelle istruzioni del progetto
(`pylotaxis-istruzioni-parte-A.md`).

## Prerequisiti (Windows)

| Strumento                                                                    | Perché                                                               | Controllo          |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------- | ------------------ |
| Node 24 LTS                                                                  | Frontend e CLI di Tauri                                              | `node -v`          |
| Rust tramite rustup                                                          | Installa da solo la versione fissata in `rust-toolchain.toml`        | `rustup --version` |
| Visual Studio Build Tools, carico "Sviluppo di applicazioni desktop con C++" | Compilatore e linker MSVC                                            | —                  |
| Strawberry Perl                                                              | OpenSSL, dentro SQLCipher, si compila con un Perl nativo per Windows | `perl -v`          |
| WebView2                                                                     | Motore del webview; già presente su Windows 11                       | —                  |

NASM non serve: senza, OpenSSL si compila senza le ottimizzazioni in assembly.
La prima compilazione Rust è lunga perché compila SQLCipher e OpenSSL dai sorgenti; le successive
usano la cache.

## Comandi

```powershell
npm ci                 # dipendenze esatte dal package-lock.json
npm run tauri dev      # sviluppo; da lanciare da solo, Ctrl+C per uscire
npm run verify         # tutte le verifiche, come in CI
npm run tauri build    # installer NSIS in target/release/bundle/nsis/
```

`npm run verify` esegue in sequenza: controllo delle versioni allineate, controllo del nome,
type-check, ESLint, Prettier, Vitest, rustfmt, Clippy con `-D warnings`, `cargo test`.

## Struttura

```text
crates/domain/        Regole e persistenza (SQLCipher, migrazioni), senza Tauri
  migrations/         Migrazioni SQL numerate: una migrazione rilasciata non si modifica mai
src-tauri/            Guscio Tauri: comandi sottili, capability, configurazione, icone
src/                  Interfaccia React: presenta, non decide
  i18n/               Italiano (riferimento) e inglese, con test di parità
  styles/tokens.css   Token dell'identità: unica fonte di colori e tipografia
scripts/              Controlli di versioni e nome, generazione delle icone
branding/             Sorgenti SVG del simbolo e dell'icona
```

## Regole che il codice fa rispettare

- **Comandi chiusi per default.** Un comando Tauri nuovo va elencato in `src-tauri/build.rs` e
  concesso in `src-tauri/capabilities/default.json`; altrimenti Tauri lo rifiuta.
- **Nome in punti noti.** Il nome del prodotto compare solo nelle costanti `APP_NAME`
  (`src/constants/app.ts`, `crates/domain/src/lib.rs`) e nei manifest; `npm run check:name` lo verifica.
- **Versione unica.** `package.json`, `Cargo.toml` alla radice e `src-tauri/tauri.conf.json` riportano
  la stessa versione; `npm run check:versions` lo verifica.
- **CSP rigida solo in release.** In `tauri dev` la pagina arriva dal server di Vite senza CSP:
  ciò che dipende dalla CSP si prova con `npm run tauri build`.

## Icone

Sorgente: `branding/app-icon.svg`. Per rigenerarle:

```powershell
node scripts/build-icons.mjs
```

Lo script disegna le dimensioni da 16 a 48 px sulla griglia dei pixel e ricava le altre dal
vettoriale. `npm run tauri icon` da solo produrrebbe icone piccole sfocate e senza i 20 e 40 px
usati da Windows con la scala al 125% e al 150%.

## Licenze dei font

Marcellus e Instrument Sans sono distribuiti con licenza SIL Open Font License 1.1: i testi delle
licenze sono in `src/assets/fonts/`. I file sono convertiti in WOFF2 senza modificare i glifi.
