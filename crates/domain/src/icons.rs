//! Icone delle app (v0.7.0): regole pure e persistenza.
//!
//! Il guscio estrae l'icona dagli eseguibili con le API di Windows e passa qui i pixel; le
//! immagini scelte dall'utente arrivano come file PNG. Qui si decide che cosa è un'icona
//! valida e come si conserva. Il webview riceve solo l'immagine, mai il percorso.

use rusqlite::params;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::registry::parse_uuid;
use crate::{Database, Error};

/// Peso massimo di un'immagine scelta dall'utente.
pub const ICON_MAX_BYTES: usize = 256 * 1024;
/// Lato massimo in pixel: oltre, l'immagine pesa in memoria senza migliorare una tessera.
pub const ICON_MAX_SIDE: u32 = 1024;
/// Lato dell'icona estratta da un eseguibile: nitida fino al 200% di scala per una tessera
/// da 48 pixel.
pub const EXE_ICON_SIDE: u32 = 96;
/// Memoria massima per decodificare un PNG durante la verifica.
const DECODE_LIMIT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    /// Estratta dall'eseguibile registrato.
    Executable,
    /// Scelta dall'utente: nessuna estrazione la sostituisce.
    Custom,
}

impl IconKind {
    fn code(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::Custom => "custom",
        }
    }
}

/// Verifica un PNG per intero, decodificandolo: firma, dimensioni e dati. Un file che
/// dichiara di essere un PNG ma non lo è viene rifiutato qui, non dal webview.
pub fn validate_png(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > ICON_MAX_BYTES {
        return Err(Error::IconTooLarge);
    }
    let decoder = png::Decoder::new_with_limits(
        std::io::Cursor::new(bytes),
        png::Limits {
            bytes: DECODE_LIMIT_BYTES,
        },
    );
    let mut reader = decoder.read_info().map_err(|_| Error::IconInvalid)?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 {
        return Err(Error::IconInvalid);
    }
    if width > ICON_MAX_SIDE || height > ICON_MAX_SIDE {
        return Err(Error::IconTooLarge);
    }
    let mut frame = vec![0; reader.output_buffer_size()];
    reader
        .next_frame(&mut frame)
        .map_err(|_| Error::IconInvalid)?;
    Ok(())
}

/// Codifica pixel RGBA in PNG.
pub fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, Error> {
    let expected = usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| Error::IconInvalid)?;
    if width == 0 || height == 0 || rgba.len() != expected {
        return Err(Error::IconInvalid);
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|_| Error::IconInvalid)?;
    writer
        .write_image_data(rgba)
        .map_err(|_| Error::IconInvalid)?;
    writer.finish().map_err(|_| Error::IconInvalid)?;
    Ok(out)
}

/// Converte i pixel BGRA che dà Windows in RGBA. Le icone vecchie non hanno canale alfa: in
/// quel caso la trasparenza viene dalla maschera, dove il nero vuol dire opaco.
pub fn icon_rgba(bgra: &[u8], mask_bgra: Option<&[u8]>) -> Vec<u8> {
    // Pixel come array da 4 byte: un eventuale resto incompleto si scarta. `as_chunks` al
    // posto di `chunks_exact(4)`, come chiede Clippy 1.98 (lezione 57). (v0.7.0)
    let (pixels, _) = bgra.as_chunks::<4>();
    let mask = mask_bgra.map(|mask| mask.as_chunks::<4>().0);
    let has_alpha = pixels.iter().any(|pixel| pixel[3] != 0);
    pixels
        .iter()
        .enumerate()
        .flat_map(|(index, pixel)| {
            let alpha = if has_alpha {
                pixel[3]
            } else {
                match mask.and_then(|mask| mask.get(index)) {
                    Some([0, ..]) | None => 255,
                    Some(_) => 0,
                }
            };
            [pixel[2], pixel[1], pixel[0], alpha]
        })
        .collect()
}

/// Revisione di un'icona: cambia quando cambia l'immagine.
pub fn icon_rev(png: &[u8]) -> String {
    crate::hex::lower(&Sha256::digest(png)[..8])
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 standard con riempimento (RFC 4648): una funzione breve al posto di un crate.
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (position, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if position <= chunk.len() {
                out.push(char::from(BASE64[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// L'icona come la mostra il webview: un'immagine autonoma, senza riferimenti a file.
pub fn png_data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64(png))
}

impl Database {
    /// Salva l'icona di un'app. Un'icona estratta non sostituisce mai una scelta dall'utente:
    /// restituisce `false` quando non ha scritto nulla per questo motivo.
    pub fn set_app_icon(&self, id: Uuid, png: &[u8], kind: IconKind) -> Result<bool, Error> {
        if kind == IconKind::Custom {
            validate_png(png)?;
        }
        let changed = self.conn.execute(
            "UPDATE apps SET icon = ?2, icon_kind = ?3, icon_rev = ?4
             WHERE id = ?1 AND (?3 = 'custom' OR icon_kind IS NULL OR icon_kind = 'executable')",
            params![id.to_string(), png, kind.code(), icon_rev(png)],
        )?;
        if changed == 0 {
            // O l'app non esiste, o ha un'icona scelta dall'utente.
            self.app(id)?;
            return Ok(false);
        }
        Ok(true)
    }

    /// Toglie l'icona: l'app torna alle iniziali finché non se ne estrae o sceglie un'altra.
    pub fn clear_app_icon(&self, id: Uuid) -> Result<(), Error> {
        let changed = self.conn.execute(
            "UPDATE apps SET icon = NULL, icon_kind = NULL, icon_rev = NULL WHERE id = ?1",
            params![id.to_string()],
        )?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Tutte le icone salvate, per id.
    pub fn app_icons(&self) -> Result<Vec<(Uuid, Vec<u8>)>, Error> {
        let mut statement = self
            .conn
            .prepare("SELECT id, icon FROM apps WHERE icon IS NOT NULL")?;
        let icons = statement
            .query_map([], |row| Ok((parse_uuid(row, 0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(icons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tests::{open_db, web, write_exe};
    use crate::registry::{AppInput, Target};

    fn square(side: u32) -> Vec<u8> {
        let rgba: Vec<u8> = (0..side * side).flat_map(|_| [10, 20, 30, 255]).collect();
        encode_rgba_png(side, side, &rgba).unwrap()
    }

    #[test]
    fn a_real_png_passes_and_everything_else_is_refused() {
        validate_png(&square(16)).unwrap();
        assert!(matches!(
            validate_png(b"not a png"),
            Err(Error::IconInvalid)
        ));
        let png = square(16);
        assert!(matches!(
            validate_png(&png[..png.len() / 2]),
            Err(Error::IconInvalid)
        ));
        let wide: Vec<u8> = (0..1100).flat_map(|_| [0, 0, 0, 0]).collect();
        assert!(matches!(
            validate_png(&encode_rgba_png(1100, 1, &wide).unwrap()),
            Err(Error::IconTooLarge)
        ));
        assert!(matches!(
            validate_png(&vec![0; ICON_MAX_BYTES + 1]),
            Err(Error::IconTooLarge)
        ));
        assert!(matches!(
            encode_rgba_png(2, 2, &[0; 3]),
            Err(Error::IconInvalid)
        ));
    }

    #[test]
    fn windows_pixels_become_rgba_and_the_mask_supplies_missing_alpha() {
        assert_eq!(icon_rgba(&[1, 2, 3, 200], None), [3, 2, 1, 200]);
        let no_alpha = [1, 2, 3, 0, 4, 5, 6, 0];
        let mask = [0, 0, 0, 0, 255, 255, 255, 0];
        assert_eq!(
            icon_rgba(&no_alpha, Some(&mask)),
            [3, 2, 1, 255, 6, 5, 4, 0]
        );
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(base64(input.as_bytes()), expected, "{input}");
        }
        assert!(png_data_url(&[1]).starts_with("data:image/png;base64,"));
    }

    #[test]
    fn an_extracted_icon_never_replaces_a_chosen_one() {
        let (_dir, db) = open_db();
        let id = db.create_app(&web("Sito")).unwrap().id;
        assert_eq!(db.app(id).unwrap().icon_rev, None);
        assert!(
            db.set_app_icon(id, &square(8), IconKind::Executable)
                .unwrap()
        );
        let first = db.app(id).unwrap().icon_rev.unwrap();
        assert!(db.set_app_icon(id, &square(12), IconKind::Custom).unwrap());
        let chosen = db.app(id).unwrap().icon_rev.unwrap();
        assert_ne!(first, chosen);
        assert!(
            !db.set_app_icon(id, &square(8), IconKind::Executable)
                .unwrap()
        );
        assert_eq!(db.app(id).unwrap().icon_rev.unwrap(), chosen);
        assert_eq!(db.app_icons().unwrap(), [(id, square(12))]);
        db.clear_app_icon(id).unwrap();
        assert!(db.app_icons().unwrap().is_empty());
        assert!(matches!(
            db.set_app_icon(Uuid::now_v7(), &square(8), IconKind::Executable),
            Err(Error::NotFound)
        ));
        assert!(matches!(
            db.set_app_icon(id, b"no", IconKind::Custom),
            Err(Error::IconInvalid)
        ));
    }

    #[test]
    fn leaving_the_executable_drops_its_icon_but_keeps_a_chosen_one() {
        let (dir, db) = open_db();
        let exe = write_exe(dir.path(), "tool.exe", b"abc");
        let info = crate::registry::inspect_executable(&exe).unwrap();
        let input = AppInput {
            target: Target::Executable {
                path: info.path,
                sha256: info.sha256,
            },
            ..web("Strumento")
        };
        let id = db.create_app(&input).unwrap().id;
        db.set_app_icon(id, &square(8), IconKind::Executable)
            .unwrap();
        db.update_app(id, &web("Strumento")).unwrap();
        assert_eq!(
            db.app(id).unwrap().icon_rev,
            None,
            "icona dell'eseguibile tolta"
        );
        db.set_app_icon(id, &square(8), IconKind::Custom).unwrap();
        db.update_app(id, &web("Strumento")).unwrap();
        assert!(
            db.app(id).unwrap().icon_rev.is_some(),
            "icona scelta tenuta"
        );
    }
}
