//! Windows Hello, inattività di sistema e stato della sessione, con le API di Windows (v0.3.0).
//!
//! È l'unico file del progetto con codice `unsafe`: ogni blocco chiama una funzione Win32 o
//! COM che windows-rs dichiara `unsafe`, e porta un commento SAFETY. Il permesso è concesso
//! funzione per funzione, non all'intero modulo.

use std::ffi::c_void;
use std::sync::Mutex;

use tauri::{Runtime, WebviewWindow};
use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::RemoteDesktop::{
    WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK, WTSDisconnected,
    WTSFreeMemory, WTSINFOEXW, WTSQuerySessionInformationW, WTSSessionInfoEx,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::core::{HSTRING, PWSTR, factory};
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
