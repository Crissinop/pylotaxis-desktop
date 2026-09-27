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

## Segreti

Dalla v0.4.0 ogni app può avere dei segreti (password, token), da **Segreti** nella sua riga. Il valore
si scrive una volta e poi non si vede più: si copia negli appunti o si sostituisce.

| Cosa                         | Dove e come                                                                                                                                                                                                                         |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Valore                       | Nel Credential Manager di Windows, una credenziale per segreto (`secret:<id>.com.crissinop.pylotaxis`), con persistenza locale. Massimo 2560 byte                                                                                   |
| Etichetta, nome utente, data | Nel registro cifrato. Del valore il registro non ha nemmeno una colonna                                                                                                                                                             |
| Copia                        | Rust mette il valore negli appunti con i formati che lo escludono dalla cronologia (Win+V) e dalla sincronizzazione tra dispositivi, e li svuota dopo 30 secondi, al blocco e alla chiusura, solo se contengono ancora quella copia |

Nessun comando restituisce il valore al webview, nemmeno in parte. Eliminare un'app elimina anche i suoi
segreti; all'avvio si eliminano le credenziali rimaste orfane. Se sposti altrove `registry.db` per
ricominciare da zero, anche i valori dei segreti vengono eliminati al primo avvio.

## Blocco

Dalla v0.3.0 il registro si può proteggere con un PIN, da **Impostazioni** nella barra in alto.
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
esecuzione nella tua sessione di Windows, che può leggere la chiave dal Credential Manager; lo
stesso limite vale per i segreti (v0.4.0).

Se dimentichi il PIN non c'è un recupero: il registro è intatto ma non si sblocca. Il backup e il
recupero arrivano con la v0.7.0.

Da provare a mano sulla macchina, perché dipende da Windows e non da un test automatico: il prompt di
Windows Hello davanti alla finestra dell'app, Win+L e il ritorno dalla sospensione (l'app deve
risultare bloccata), una sessione di Desktop remoto scollegata.

## Rapidità

Dalla v0.5.0 ogni app si apre senza mouse, e il portale vive nella tray.

| Cosa               | Come                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Palette di comando | Ctrl+Alt+Spazio da qualunque app; in **Impostazioni** si può scegliere Ctrl+Maiusc+Spazio o Ctrl+Alt+P. Cerca app (per nome, tag, categoria), segreti come "App · Etichetta" (Invio copia il valore, come il pulsante "Copia") e le azioni Blocca e Apri. Frecce, Invio, Esc; si chiude da sola quando perde il fuoco e dopo un'azione, e la stessa scorciatoia la richiude |
| Tray               | Clic sinistro: la finestra. Menu con il destro: Apri, Palette di comando, Blocca, Esci. Chiudere la finestra (pulsante o Alt+F4) la nasconde; si esce da **Esci**                                                                                                                                                                                                           |
| Istanza singola    | Avviare l'app una seconda volta porta davanti quella già aperta                                                                                                                                                                                                                                                                                                             |
| Link diretti       | `pylotaxis://app/<id>` apre la palette con l'app selezionata: **l'avvio chiede sempre Invio**, perché qualunque pagina web può aprire un link. Solo nella forma esatta; id sconosciuti si ignorano. Lo schema lo registra l'installer; in sviluppo lo registra `tauri dev`                                                                                                  |
| Barra del titolo   | Propria: si trascina dall'intestazione, doppio clic per ingrandire, pulsanti riduci, ingrandisci e chiudi                                                                                                                                                                                                                                                                   |

Da bloccata la scorciatoia e la voce Palette della tray portano alla schermata di blocco, la palette si
nasconde e i link si scartano. Se un'altra app usa già la scorciatoia scelta, Windows non la concede:
le impostazioni lo dicono e resta attiva la precedente. Con la barra propria non compare il menu di
Windows 11 sopra il pulsante Ingrandisci. Sulla tastiera italiana AltGr equivale a Ctrl+Alt, quindi
anche AltGr+Spazio apre la palette: in quel caso conviene Ctrl+Maiusc+Spazio.

## Organizzazione

Dalla v0.6.0 le app hanno un ambiente, si raccolgono in gruppi e, se lo chiedi, mostrano se rispondono.

| Cosa              | Come                                                                                                                                                                                                                                                                                                                                                                                                       |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ambienti          | Nessuno, sviluppo, collaudo o produzione, nella scheda dell'app. Lo stesso nome può esistere una volta per ambiente. La **produzione** porta l'etichetta PRODUZIONE e un filetto bronzo, ovunque compaia: elenco, palette, gruppi, finestre di conferma                                                                                                                                                    |
| Gruppi di avvio   | Un nome e fino a 20 app in ordine di avvio. Dalla finestra principale si vede l'elenco prima di avviare; dalla palette ("Gruppo · Nome") parte con Invio. Un eseguibile cambiato non parte in blocco: si conferma uno per uno nell'esito, che compare nella finestra principale anche quando il gruppo parte dalla palette                                                                                 |
| Stato delle app   | Solo per le app web, acceso app per app (spento per default). Una richiesta senza credenziali né cookie, senza seguire reindirizzamenti e senza proxy, al massimo una volta al minuto per app e solo con la finestra in vista; mai da bloccata. Esiti: attiva (risposta sotto 500), errore del server, non raggiungibile, certificato non valido. Il TLS è quello di Windows, con i certificati di sistema |
| Avvio con Windows | In **Impostazioni**, spento per default. All'accesso l'app parte nella tray, senza finestra                                                                                                                                                                                                                                                                                                                |

La migrazione 0005 aggiunge ambienti e gruppi: dopo il primo avvio della v0.6.0 le build precedenti non
aprono più il registro. Prima dell'aggiornamento conviene copiare `registry.db`. Dietro un proxy
aziendale le app esterne possono risultare non raggiungibili: il controllo si collega direttamente.

## Struttura

```text
crates/domain/        Regole e persistenza (SQLCipher, migrazioni), senza Tauri
  migrations/         Migrazioni SQL numerate: una migrazione rilasciata non si modifica mai
src-tauri/            Guscio Tauri: comandi sottili, capability, configurazione, icone
  app_commands.rs     Elenco unico dei comandi: permessi (build.rs) e test del blocco
  src/lock.rs         Filtro del blocco davanti a ogni comando, controllo automatico
  src/platform/       Windows Hello, inattività, sessione e appunti: l'unico codice `unsafe`
  src/vault.rs        Valori dei segreti nel Credential Manager: nessuna funzione li restituisce
  src/clipboard.rs    Svuotamento automatico degli appunti
  src/palette.rs      Finestra della palette: apertura, chiusura, blocco
  src/shortcut.rs     Scorciatoia globale, con ritorno alla precedente se Windows rifiuta
  src/tray.rs         Icona nella tray, menu, chiusura verso la tray
  src/links.rs        Link diretti: forma esatta, mai un avvio senza Invio
  src/groups.rs       Gruppi di avvio: ogni app passa dall'unica funzione di avvio
  src/health.rs       Stato delle app web: richieste solo con la finestra in vista
  src/autostart.rs    Avvio con Windows, stato letto dal sistema
  capabilities/       Una capability per finestra (principale e palette), con elenchi esatti
src/                  Interfaccia React: presenta, non decide
  features/registry/  Elenco, modulo di inserimento e sezioni per categoria
  features/lock/      Schermata di blocco (solo presentazione)
  features/security/  Impostazioni di sicurezza e finestra del PIN
  features/secrets/   Segreti di un'app: elenco, modulo in sola scrittura, copia
  features/palette/   Palette di comando e sua classifica (funzione pura, con test)
  features/settings/  Scorciatoia e avvio con Windows
  features/groups/    Gruppi: modulo, conferma, esito; ordine come funzioni pure
  features/health/    Stato delle app web
  components/         Simbolo, controlli della barra del titolo, etichetta dell'ambiente
  lib/                Ponte verso i comandi Rust, codici d'errore, tag
  i18n/               Italiano (riferimento) e inglese, con test di parità
  styles/tokens.css   Token dell'identità: unica fonte di colori e tipografia
scripts/              Controlli di versioni e nome, generazione delle icone
branding/             Sorgenti SVG del simbolo e dell'icona
```

## Regole che il codice fa rispettare

- **Comandi chiusi per default.** Un comando Tauri nuovo va elencato in `src-tauri/app_commands.rs`
  e concesso nella capability della finestra che lo usa (`default.json` per la principale,
  `palette.json` per la palette); altrimenti Tauri lo rifiuta. Un test confronta gli elenchi
  finestra per finestra. I plugin (dialog, opener, scorciatoia, link, istanza singola) si usano solo
  da Rust: nessuna capability ne concede i comandi.
- **Bloccata vuol dire bloccata.** Da bloccata passano solo `app_info`, `lock_status`, `unlock_pin`,
  `unlock_hello`, `tray_setup` (etichette della tray) e `palette_ready` (nessun dato); un comando nuovo nasce rifiutato finché non lo si aggiunge di proposito a
  `ALLOWED_WHILE_LOCKED` (`src-tauri/src/lock.rs`). Nessun comando restituisce il PIN o la sua impronta.
- **Nessun percorso dal webview.** Un eseguibile si registra solo con la finestra di scelta di
  Windows, aperta da Rust; il frontend riceve un gettone monouso. Si avvia per id, e un
  eseguibile cambiato dall'ultima conferma non parte senza un nuovo consenso.
- **Nome in punti noti.** Il nome del prodotto compare solo nelle costanti `APP_NAME`
  (`src/constants/app.ts`, `crates/domain/src/lib.rs`), nei manifest e, in minuscolo, come schema dei
  link in `tauri.conf.json`, da cui Rust lo legge; `npm run check:name` lo verifica.
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
