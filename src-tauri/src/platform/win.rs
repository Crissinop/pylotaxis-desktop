//! Windows Hello, inattività di sistema, stato della sessione (v0.3.0) e appunti privati
//! (v0.4.0), con le API di Windows.
//!
//! È l'unico file del progetto con codice `unsafe`: ogni blocco chiama una funzione Win32 o
//! COM che windows-rs dichiara `unsafe`, e porta un commento SAFETY. Il permesso è concesso
//! funzione per funzione, non all'intero modulo.

use std::ffi::c_void;

use super::IconPixels;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{Runtime, WebviewWindow};
use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
use windows::Win32::Foundation::GlobalFree;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::RemoteDesktop::{
    WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK, WTSDisconnected,
    WTSFreeMemory, WTSINFOEXW, WTSQuerySessionInformationW, WTSSessionInfoEx,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::core::{HSTRING, PCWSTR, PWSTR, factory, w};
use windows_future::{AsyncStatus, IAsyncOperation};

use super::{availability_result, verification_result};
use crate::errors::CommandError;

type Verification = IAsyncOperation<UserConsentVerificationResult>;

/// Richiesta a Windows Hello in corso. Una alla volta (A.7.5, lezione 11): una nuova annulla
/// quella rimasta appesa, e uno sblocco riuscito chiude quella ancora aperta.
static PENDING: Mutex<Option<Verification>> = Mutex::new(None);

pub fn window_owner<R: Runtime>(window: &WebviewWindow<R>) -> Result<usize, CommandError> {
    window
        .hwnd()
        .map(|hwnd| hwnd.0.expose_provenance())
        .map_err(|_| CommandError::HELLO_FAILED)
}

pub fn hello_availability() -> Result<(), CommandError> {
    let availability = UserConsentVerifier::CheckAvailabilityAsync()
        .and_then(|operation| operation.get())
        .map_err(|_| CommandError::HELLO_FAILED)?;
    availability_result(availability.0)
}

/// Chiede a Windows Hello di verificare la presenza dell'utente, con il prompt legato alla
/// finestra `owner`: così resta davanti alla finestra dell'app invece di aprirsi dietro.
/// Blocca il thread fino alla risposta: va chiamata da `spawn_blocking`.
#[allow(unsafe_code)]
pub fn hello_verify(owner: usize, message: &str) -> Result<(), CommandError> {
    let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>()
        .map_err(|_| CommandError::HELLO_FAILED)?;
    let operation = {
        let mut pending = PENDING
            .lock()
            .map_err(|_| CommandError::STATE_UNAVAILABLE)?;
        if let Some(previous) = pending.take() {
            let _ = previous.Cancel();
        }
        let window = HWND(std::ptr::with_exposed_provenance_mut::<c_void>(owner));
        // SAFETY: `window` è l'HWND della finestra principale, letto da Tauri subito prima;
        // la finestra vive quanto l'app. `message` resta valido per tutta la chiamata.
        // L'operazione restituita è un'interfaccia COM di cui windows-rs conta i riferimenti.
        let operation: Verification =
            unsafe { interop.RequestVerificationForWindowAsync(window, &HSTRING::from(message)) }
                .map_err(|_| CommandError::HELLO_FAILED)?;
        *pending = Some(operation.clone());
        operation
    };

    let outcome = operation.get();
    if let Ok(mut pending) = PENDING.lock()
        && pending.as_ref() == Some(&operation)
    {
        *pending = None;
    }
    match outcome {
        Ok(result) => verification_result(result.0),
        Err(_) if operation.Status().ok() == Some(AsyncStatus::Canceled) => {
            Err(CommandError::HELLO_CANCELED)
        }
        Err(_) => Err(CommandError::HELLO_FAILED),
    }
}

pub fn hello_cancel_pending() {
    if let Ok(mut pending) = PENDING.lock()
        && let Some(operation) = pending.take()
    {
        let _ = operation.Cancel();
    }
}

/// Millisecondi dall'ultimo input dell'utente in questa sessione di Windows (tastiera, mouse,
/// tocco), in qualunque applicazione.
#[allow(unsafe_code)]
pub fn idle_ms() -> Option<u64> {
    let mut info = LASTINPUTINFO {
        cbSize: u32::try_from(size_of::<LASTINPUTINFO>()).ok()?,
        dwTime: 0,
    };
    // SAFETY: `info` è una struttura valida sullo stack con `cbSize` impostato, come chiede l'API.
    let read = unsafe { GetLastInputInfo(&mut info) };
    if !read.as_bool() {
        return None;
    }
    // SAFETY: nessun argomento; legge soltanto il contatore dei millisecondi del sistema.
    let now = unsafe { GetTickCount() };
    // I due contatori sono a 32 bit e ripartono da zero ogni 49,7 giorni: la differenza con
    // `wrapping_sub` resta giusta anche a cavallo del giro.
    Some(u64::from(now.wrapping_sub(info.dwTime)))
}

/// Vero se la sessione di Windows è bloccata o scollegata (desktop remoto chiuso).
#[allow(unsafe_code)]
pub fn session_locked() -> Option<bool> {
    let mut buffer = PWSTR::null();
    let mut bytes = 0_u32;
    // SAFETY: i due puntatori d'uscita sono variabili locali valide. Il buffer restituito
    // appartiene a Windows e si libera qui sotto con WTSFreeMemory.
    let queried = unsafe {
        WTSQuerySessionInformationW(
            Some(WTS_CURRENT_SERVER_HANDLE),
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        )
    };
    queried.ok()?;
    let locked = read_session_info(buffer, bytes);
    // SAFETY: `buffer` viene da WTSQuerySessionInformationW e dopo questa riga non si usa più.
    unsafe { WTSFreeMemory(buffer.0.cast()) };
    locked
}

#[allow(unsafe_code)]
fn read_session_info(buffer: PWSTR, bytes: u32) -> Option<bool> {
    let complete = usize::try_from(bytes).ok()? >= size_of::<WTSINFOEXW>();
    if buffer.is_null() || !complete {
        return None;
    }
    // SAFETY: il buffer non è nullo, contiene almeno una WTSINFOEXW (controllato sopra) ed è
    // allineato, perché Windows lo alloca per quel tipo.
    let info = unsafe { &*buffer.0.cast::<WTSINFOEXW>() };
    if info.Level != 1 {
        return None;
    }
    // SAFETY: con `Level` pari a 1 il campo valido dell'unione è `WTSInfoExLevel1`.
    let level1 = unsafe { info.Data.WTSInfoExLevel1 };
    let flag_locked = u32::try_from(level1.SessionFlags).ok() == Some(WTS_SESSIONSTATE_LOCK);
    Some(flag_locked || level1.SessionState == WTSDisconnected)
}

/// Tentativi di aprire gli appunti, che un'altra app può tenere aperti per un istante.
const CLIPBOARD_OPEN_ATTEMPTS: u32 = 10;
const CLIPBOARD_RETRY: Duration = Duration::from_millis(15);

/// Formati che tengono il contenuto fuori dalla cronologia (Win+V) e dalla sincronizzazione
/// tra dispositivi. I primi tre sono documentati da Microsoft (Clipboard Formats); l'ultimo è
/// una convenzione dei gestori di appunti di terze parti. Ognuno riceve un DWORD pari a 0.
fn private_formats() -> [PCWSTR; 4] {
    [
        w!("ExcludeClipboardContentFromMonitorProcessing"),
        w!("CanIncludeInClipboardHistory"),
        w!("CanUploadToCloudClipboard"),
        w!("Clipboard Viewer Ignore"),
    ]
}

#[allow(unsafe_code)]
fn open_clipboard(owner: Option<HWND>) -> Result<(), CommandError> {
    for attempt in 1..=CLIPBOARD_OPEN_ATTEMPTS {
        // SAFETY: `owner` è l'HWND della finestra principale oppure nessuno; nessun puntatore.
        if unsafe { OpenClipboard(owner) }.is_ok() {
            return Ok(());
        }
        if attempt < CLIPBOARD_OPEN_ATTEMPTS {
            std::thread::sleep(CLIPBOARD_RETRY);
        }
    }
    Err(CommandError::CLIPBOARD_UNAVAILABLE)
}

/// Mette `bytes` negli appunti già aperti, nel formato `format`.
#[allow(unsafe_code)]
fn set_clipboard_bytes(format: u32, bytes: &[u8]) -> Result<(), CommandError> {
    // SAFETY: la memoria è allocata qui, mobile e della dimensione di `bytes`; la copia resta
    // entro quella dimensione. Se SetClipboardData riesce la memoria passa al sistema,
    // altrimenti la si libera qui.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len())
            .map_err(|_| CommandError::CLIPBOARD_UNAVAILABLE)?;
        let target = GlobalLock(memory);
        if target.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err(CommandError::CLIPBOARD_UNAVAILABLE);
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target.cast::<u8>(), bytes.len());
        // Restituisce un errore anche quando riesce (contatore a zero): si ignora.
        let _ = GlobalUnlock(memory);
        if SetClipboardData(format, Some(HANDLE(memory.0))).is_err() {
            let _ = GlobalFree(Some(memory));
            return Err(CommandError::CLIPBOARD_UNAVAILABLE);
        }
    }
    Ok(())
}

/// Copia `text` negli appunti come testo privato e restituisce il numero di sequenza che
/// gli appunti hanno subito dopo: serve a svuotarli solo se nessuno ha copiato altro.
#[allow(unsafe_code)]
pub fn clipboard_copy_private(owner: usize, text: &str) -> Result<u32, CommandError> {
    let window = HWND(std::ptr::with_exposed_provenance_mut::<c_void>(owner));
    let mut bytes: Vec<u8> = text
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_ne_bytes)
        .collect();
    open_clipboard(Some(window))?;
    let written = (|| {
        // SAFETY: gli appunti sono aperti da questo thread (sopra) e si chiudono qui sotto.
        unsafe { EmptyClipboard() }.map_err(|_| CommandError::CLIPBOARD_UNAVAILABLE)?;
        // Prima i formati privati, poi il testo: se uno manca, il testo non entra affatto.
        for name in private_formats() {
            // SAFETY: `name` è una stringa costante terminata da zero (macro `w!`).
            let format = unsafe { RegisterClipboardFormatW(name) };
            if format == 0 {
                return Err(CommandError::CLIPBOARD_UNAVAILABLE);
            }
            set_clipboard_bytes(format, &0_u32.to_ne_bytes())?;
        }
        set_clipboard_bytes(u32::from(CF_UNICODETEXT.0), &bytes)
    })();
    if written.is_err() {
        // SAFETY: appunti ancora aperti da questo thread: nulla di parziale resta dentro.
        let _ = unsafe { EmptyClipboard() };
    }
    // SAFETY: chiude gli appunti aperti da questo thread.
    let _ = unsafe { CloseClipboard() };
    // Copia locale del valore: la si azzera appena non serve più.
    bytes.fill(0);
    written?;
    // SAFETY: nessun argomento; legge il contatore degli appunti.
    Ok(unsafe { GetClipboardSequenceNumber() })
}

/// Svuota gli appunti se il loro numero di sequenza è ancora `sequence`.
#[allow(unsafe_code)]
pub fn clipboard_clear_if(sequence: u32) {
    // SAFETY: nessun argomento.
    if unsafe { GetClipboardSequenceNumber() } != sequence || open_clipboard(None).is_err() {
        return;
    }
    // SAFETY: appunti aperti da questo thread; il controllo si ripete a appunti aperti, quando
    // nessun altro può cambiarli.
    unsafe {
        if GetClipboardSequenceNumber() == sequence {
            let _ = EmptyClipboard();
        }
        let _ = CloseClipboard();
    }
}

/// Pixel dell'icona di un eseguibile al lato richiesto, in BGRA dall'alto in basso, con la
/// maschera per le icone senza canale alfa. `None` se il file non ha un'icona propria. (v0.7.0)
#[allow(unsafe_code)]
pub fn executable_icon_pixels(path: &std::path::Path, side: u32) -> Option<IconPixels> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::Shell::SHDefExtractIconW;
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};
    use windows::core::PCWSTR;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut icon = HICON::default();
    // SAFETY: `wide` è terminata da zero e vive per tutta la chiamata; `icon` è una variabile
    // d'uscita sullo stack; l'icona piccola non si chiede.
    let result =
        unsafe { SHDefExtractIconW(PCWSTR(wide.as_ptr()), 0, 0, Some(&mut icon), None, side) };
    // S_FALSE (nessuna icona nel file) è un successo con l'icona vuota.
    if result.is_err() || icon.is_invalid() {
        return None;
    }
    let pixels = icon_pixels(icon);
    // SAFETY: l'icona è stata creata qui sopra per questa chiamata e non è condivisa.
    let _ = unsafe { DestroyIcon(icon) };
    pixels
}

/// Colore e maschera di un'icona; le bitmap che Windows crea per `GetIconInfo` si liberano qui.
#[allow(unsafe_code)]
fn icon_pixels(icon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<IconPixels> {
    use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut info = ICONINFO::default();
    // SAFETY: `icon` è valida; `info` è una struttura d'uscita sullo stack.
    unsafe { GetIconInfo(icon, &mut info) }.ok()?;
    let color = bitmap_bgra(info.hbmColor);
    let mask = bitmap_bgra(info.hbmMask);
    for bitmap in [info.hbmColor, info.hbmMask] {
        if !bitmap.is_invalid() {
            // SAFETY: bitmap create da `GetIconInfo` per chi chiama, che deve liberarle.
            let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        }
    }
    // Le icone monocromatiche non hanno la bitmap del colore: niente icona, restano le iniziali.
    let (width, height, bgra) = color?;
    let mask = mask
        .filter(|(mask_width, mask_height, _)| (*mask_width, *mask_height) == (width, height))
        .map(|(_, _, pixels)| pixels);
    Some(IconPixels {
        width,
        height,
        bgra,
        mask,
    })
}

/// Pixel di una bitmap in BGRA a 32 bit, dall'alto in basso.
#[allow(unsafe_code)]
fn bitmap_bgra(bitmap: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        GetDIBits, GetObjectW, HGDIOBJ,
    };

    if bitmap.is_invalid() {
        return None;
    }
    let mut header = BITMAP::default();
    let header_size = i32::try_from(size_of::<BITMAP>()).ok()?;
    // SAFETY: `bitmap` è valida; `header` è grande esattamente `header_size` byte.
    let read = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            header_size,
            Some(std::ptr::addr_of_mut!(header).cast()),
        )
    };
    if read == 0 {
        return None;
    }
    let width = u32::try_from(header.bmWidth).ok()?;
    let height = u32::try_from(header.bmHeight).ok()?;
    if width == 0 || height == 0 || width > 1024 || height > 1024 {
        return None;
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).ok()?,
            biWidth: header.bmWidth,
            // Altezza negativa: righe dall'alto in basso, come le vuole il PNG.
            biHeight: -header.bmHeight,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0_u8; usize::try_from(width * height * 4).ok()?];
    // SAFETY: nessun argomento: un contesto di memoria compatibile con lo schermo.
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        return None;
    }
    // SAFETY: `pixels` ha spazio per `height` righe a 32 bit da `width` pixel, come descrive
    // `info`; `dc` e `bitmap` sono validi per tutta la chiamata.
    let lines = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    // SAFETY: contesto creato qui sopra e non più usato.
    let _ = unsafe { DeleteDC(dc) };
    (u32::try_from(lines).ok()? == height).then_some((width, height, pixels))
}

#[cfg(test)]
mod tests {
    use windows::Security::Credentials::UI::UserConsentVerifierAvailability as Availability;

    use super::*;

    /// La traduzione dei risultati (platform/mod.rs) usa numeri, così si prova anche su
    /// Linux; qui, su Windows, si controlla che siano quelli delle costanti WinRT.
    #[test]
    fn the_mapped_numbers_match_the_winrt_constants() {
        assert_eq!(Availability::Available.0, 0);
        assert_eq!(Availability::DeviceNotPresent.0, 1);
        assert_eq!(Availability::NotConfiguredForUser.0, 2);
        assert_eq!(Availability::DisabledByPolicy.0, 3);
        assert_eq!(Availability::DeviceBusy.0, 4);
        assert_eq!(UserConsentVerificationResult::Verified.0, 0);
        assert_eq!(UserConsentVerificationResult::DeviceNotPresent.0, 1);
        assert_eq!(UserConsentVerificationResult::RetriesExhausted.0, 5);
        assert_eq!(UserConsentVerificationResult::Canceled.0, 6);
    }

    #[test]
    fn a_missing_or_short_session_buffer_reads_as_unknown() {
        assert_eq!(read_session_info(PWSTR::null(), 4096), None);
        let mut short = [0_u16; 4];
        assert_eq!(read_session_info(PWSTR(short.as_mut_ptr()), 8), None);
    }
}
