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
npm run hooks:install  # una volta per clone: npm run verify prima di ogni push
npm run tauri dev      # sviluppo; da lanciare da solo, Ctrl+C per uscire
npm run verify         # tutte le verifiche, come in CI
npm run tauri build    # installer NSIS in target/release/bundle/nsis/
```

`npm run verify` esegue in sequenza: controllo delle versioni allineate, controllo del nome,
type-check, ESLint, Prettier, Vitest, rustfmt, Clippy con `-D warnings`, `cargo test`.

## Dati e chiave

| Cosa                         | Dove                                                                              |
| ---------------------------- | --------------------------------------------------------------------------------- |
| Registro cifrato (SQLCipher) | `%LOCALAPPDATA%\com.crissinop.pylotaxis\registry.db`                              |
| Chiave del registro          | Credential Manager di Windows, voce `com.crissinop.pylotaxis`, persistenza locale |

Entrambi restano su questa macchina: il registro contiene percorsi locali. La chiave nasce al
primo avvio e si salva **prima** che il file venga creato. Se il file esiste ma la chiave non c'è,
l'app mostra "Il registro non si apre" e non tocca i dati: non esiste un recupero senza la chiave.
Il backup cifrato arriva con la v0.7.0; fino ad allora, per ricominciare da zero, chiudi l'app e
sposta altrove `registry.db`.

## Blocco

Dalla v0.3.0 il registro si può proteggere con un PIN, da **Sicurezza** nella barra in alto.
Con il PIN impostato l'app si blocca a ogni avvio, dopo il periodo di inattività di sistema scelto
(15 minuti se non lo cambi), quando si blocca o si scollega la sessione di Windows, e con Ctrl+L.
Si sblocca con il PIN oppure, se attivato, con Windows Hello.

| Cosa                | Dove e come                                                                                                                             |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| PIN                 | Solo la sua impronta Argon2id (64 MiB, 3 passate, 4 corsie), dentro il registro cifrato. Il PIN in chiaro non si salva da nessuna parte |
| Tentativi sbagliati | 5 liberi, poi un'attesa da 30 secondi che raddoppia fino a 15 minuti. Il conteggio sta nel registro: riavviare l'app non lo azzera      |
| Windows Hello       | Un'alternativa al PIN, mai l'unico modo per entrare: si attiva solo con il PIN impostato e dopo una verifica riuscita                   |

Il blocco lo applica Rust: da bloccata ogni comando tranne lo sblocco viene rifiutato con `LOCKED`.
È un **blocco di presenza**: ferma chi siede a questo computer. Non protegge da un programma già in
esecuzione nella tua sessione di Windows, che può leggere la chiave dal Credential Manager; la
custodia separata dei segreti arriva con la v0.4.0.

Se dimentichi il PIN non c'è un recupero: il registro è intatto ma non si sblocca. Il backup e il
recupero arrivano con la v0.7.0.

Da provare a mano sulla macchina, perché dipende da Windows e non da un test automatico: il prompt di
Windows Hello davanti alla finestra dell'app, Win+L e il ritorno dalla sospensione (l'app deve
risultare bloccata), una sessione di Desktop remoto scollegata.

## Struttura

```text
crates/domain/        Regole e persistenza (SQLCipher, migrazioni), senza Tauri
  migrations/         Migrazioni SQL numerate: una migrazione rilasciata non si modifica mai
src-tauri/            Guscio Tauri: comandi sottili, capability, configurazione, icone
  app_commands.rs     Elenco unico dei comandi: permessi (build.rs) e test del blocco
  src/lock.rs         Filtro del blocco davanti a ogni comando, controllo automatico
  src/platform/       Windows Hello, inattività e sessione: l'unico codice `unsafe`
src/                  Interfaccia React: presenta, non decide
  features/registry/  Elenco, modulo di inserimento e sezioni per categoria
  features/lock/      Schermata di blocco (solo presentazione)
  features/security/  Impostazioni di sicurezza e finestra del PIN
  lib/                Ponte verso i comandi Rust, codici d'errore, tag
  i18n/               Italiano (riferimento) e inglese, con test di parità
  styles/tokens.css   Token dell'identità: unica fonte di colori e tipografia
scripts/              Controlli di versioni e nome, generazione delle icone
branding/             Sorgenti SVG del simbolo e dell'icona
```

## Regole che il codice fa rispettare

- **Comandi chiusi per default.** Un comando Tauri nuovo va elencato in `src-tauri/app_commands.rs`
  e concesso in `src-tauri/capabilities/default.json`; altrimenti Tauri lo rifiuta. Un test
  confronta i due elenchi.
- **Bloccata vuol dire bloccata.** Da bloccata passano solo `app_info`, `lock_status`, `unlock_pin` e
  `unlock_hello`; un comando nuovo nasce rifiutato finché non lo si aggiunge di proposito a
  `ALLOWED_WHILE_LOCKED` (`src-tauri/src/lock.rs`). Nessun comando restituisce il PIN o la sua impronta.
- **Nessun percorso dal webview.** Un eseguibile si registra solo con la finestra di scelta di
  Windows, aperta da Rust; il frontend riceve un gettone monouso. Si avvia per id, e un
  eseguibile cambiato dall'ultima conferma non parte senza un nuovo consenso.
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
