use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use chrono::Local;

/// Registro opcional (`--log`) con las mismas líneas que la consola, cada
/// una con marca de tiempo. Se abre en modo append: varias ejecuciones
/// pueden compartir archivo.
pub struct EventLog(Option<BufWriter<File>>);

impl EventLog {
    pub fn open(path: Option<&Path>) -> io::Result<Self> {
        let Some(path) = path else {
            return Ok(EventLog(None));
        };
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(EventLog(Some(BufWriter::new(file))))
    }

    /// Escribe una línea. El log es auxiliar: un fallo de escritura no debe
    /// interrumpir un movimiento de archivos a medias, así que se ignora.
    pub fn line(&mut self, text: &str) {
        if let Some(out) = &mut self.0 {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(out, "[{now}] {}", text.trim_start());
        }
    }
}

impl Drop for EventLog {
    fn drop(&mut self) {
        if let Some(out) = &mut self.0 {
            let _ = out.flush();
        }
    }
}
