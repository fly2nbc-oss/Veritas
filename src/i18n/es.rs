//! Spanish (Spain, neutral).
//!
//! Form of address "usted" or impersonal, quotation marks « », thousands point and decimal
//! comma, date "28/12/2026". Core terms: Verificar (Prüfen), Crear MD5 (MD5 erzeugen),
//! historial (Verlauf), ejecución (Lauf), Papelera (Papierkorb), subcarpetas (Unterordner),
//! suma de verificación (Prüfsumme). Status: pendiente, ok, distinto, falta, nuevo, dañado,
//! error. What each field shows and how much room it has is documented at `Texts` in
//! `mod.rs`; `msg.rs` explains the placeholders in `msg` (`{}`, `{n}`, `{p}`, `{m}`).
//!
//! Where a number precedes a word that agrees with it (reutilizada/reutilizadas,
//! se queda/se quedan), the text itself distinguishes singular and plural. In the templates
//! of `msg`, by contrast, the number follows a colon ("…: {n}"), because nothing there can
//! distinguish them.

use super::{Msg, Texts};
use crate::history::Change;

fn n(value: usize) -> String {
    super::number(value, ".")
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{} {many}", n(count))
    }
}

fn files(count: usize) -> String {
    plural(count, "archivo", "archivos")
}

fn checksums(count: usize) -> String {
    plural(count, "suma de verificación", "sumas de verificación")
}

fn entries(count: usize) -> String {
    plural(count, "entrada", "entradas")
}

/// "1 distinto", "2 distintos", "2 errores". "falta" is a verb: the number follows it
/// ("falta 1", "faltan 2") – "1 falta" would mean "1 mistake, 1 foul".
fn counted(change: Change, count: usize) -> String {
    let (one, many) = match change {
        Change::Differs => ("distinto", "distintos"),
        Change::Damaged => ("dañado", "dañados"),
        Change::Changed => ("modificado", "modificados"),
        Change::New => ("nuevo", "nuevos"),
        Change::Added => ("añadido", "añadidos"),
        Change::Missing => {
            return if count == 1 {
                "falta 1".to_string()
            } else {
                format!("faltan {}", n(count))
            };
        }
        Change::Dropped => ("quitado", "quitados"),
        Change::Unreadable => ("error", "errores"),
    };
    plural(count, one, many)
}

pub static TEXTS: Texts = Texts {
    date_time: "%d/%m/%Y %H:%M",
    date: "%d/%m/%Y",
    thousands: ".",
    decimal: ',',
    size_units: ["B", "KB", "MB", "GB", "TB"],
    percent: |value| format!("{value}\u{a0}%"),
    files,
    checksums,
    entries,
    runs: |count| plural(count, "ejecución", "ejecuciones"),

    key_ctrl: "Ctrl",
    key_enter: "Intro",
    key_delete: "Supr",

    about_tip: "Acerca de Veritas (F1)",
    language_tip: |name| format!("Idioma: {name} (Ctrl+L)"),
    search_hint: "Buscar",
    search_tip: "Filtrar nombres de archivo (Ctrl+F)",
    threads_tip: "Subprocesos: archivos cuyo hash se calcula a la vez (Ctrl+,; ↑/↓ cambian el \
                  número). En un disco duro (HDD) siempre es uno solo, porque allí es más \
                  rápido. Se aplica desde la próxima ejecución.",
    threads_hdd: "Esta carpeta está en un disco duro (HDD): Veritas procesa aquí un archivo tras \
                  otro, así que el número no se aplica.",
    locked: "Bloqueado durante una ejecución. Pulse Esc dos veces para cancelar.",
    subfolders_on: "Subcarpetas: activado. Verificar y Crear MD5 incluyen también los archivos de \
                    las subcarpetas. Clic: solo los archivos que están directamente en la \
                    carpeta.",
    subfolders_off: "Subcarpetas: desactivado. Verificar y Crear MD5 solo incluyen los archivos \
                     que están directamente en la carpeta; las entradas de las subcarpetas se \
                     quedan como están. Clic: incluir las subcarpetas.",
    theme_light: "Tema: claro",
    theme_dark: "Tema: oscuro",
    theme_system: "Tema: como el sistema",
    verify: "Verificar",
    verify_tip: |md5| format!("Verifica la carpeta con {md5} (F5)"),
    generate: "Crear MD5",
    generate_tip: |md5| {
        format!(
            "Escribe {md5} con las sumas de verificación de todos los archivos de la carpeta \
             (Ctrl+Intro)"
        )
    },
    md5_here: "Archivo de sumas de verificación de esta carpeta",
    md5_missing: "Aún no está en la carpeta. «Crear MD5» lo crea.",

    loading: "La carpeta aún se está leyendo. Vuelva a intentarlo en un momento.",
    writing: "Escribiendo – casi listo.",
    unread: "No se pudo leer el historial. Elija de nuevo la carpeta a la izquierda.",
    run_busy: "Ya hay una ejecución en curso. Pulse Esc dos veces para cancelarla.",
    verify_needs_md5: |md5| format!("Aún no hay {md5} en la carpeta. Primero «Crear MD5»."),

    pick_folder: "Elija una carpeta en el árbol.",
    reading_folder: |folder| format!("Leyendo «{folder}»…"),
    folder_unreadable_status: "No se pudo leer la carpeta.",
    folder_status: |md5, count, exists| {
        if exists {
            format!("{md5} · {}", files(count))
        } else {
            format!("Aún sin {md5} · {}", files(count))
        }
    },
    md5_unchanged: |md5| format!("{md5} se queda como está."),
    status_generating: |md5| format!("Escribiendo {md5}…"),
    status_verifying: |md5| format!("Verificando con {md5}…"),
    cancelling: "Cancelando…",
    row_history: "todas las ejecuciones de esta carpeta; doble clic o Ctrl+H las muestra",
    row_md5: "doble clic lo abre",
    md5_then_now: |algo, old, new| format!("{algo} guardado {old}, ahora {new}"),
    md5_then_now_lines: |algo, old, new| format!("{algo} guardado {old}\n{algo} ahora    {new}"),
    short_damaged: "contenido modificado, estructura posiblemente dañada",
    short_intact: "contenido modificado; inicio y final correctos – no se descarta un daño \
                   interior",
    short_unchecked: "contenido modificado; estructura de este tipo de archivo sin verificar",
    detail_changed_intact: "El contenido ha cambiado. El inicio y el final están correctos, pero \
                            eso no descarta un daño interior.",
    detail_changed_unchecked: "El contenido ha cambiado. Veritas no verifica la estructura de \
                               este tipo de archivo.",
    detail_damaged: "El contenido ha cambiado y es posible que la estructura esté dañada.",
    restore_help: "¿No lo modificó usted? Entonces recupérelo de la copia de seguridad.",
    structure: |reason| format!("Verificación de la estructura: {reason}"),
    detail_deleted: |md5| {
        format!(
            "Figura en {md5}, pero ya no está en la carpeta. ¿Se eliminó a propósito? Entonces \
             «Crear MD5» lo quita."
        )
    },
    detail_new: |md5| format!("Está en la carpeta, pero no figura en {md5}. «Crear MD5» lo añade."),
    detail_open: |md5| format!("Figura en {md5}; aún sin verificar. «Verificar» (F5) lo lee."),
    detail_ok: |md5| format!("Coincide con {md5}."),
    detail_error: "¿Está abierto o bloqueado en otro programa? Libérelo y vuelva a verificar \
                   (F5).",

    chip_all: |keys| format!("Mostrar todos los archivos (Esc, {keys})"),
    chip_only: |label, keys| format!("Mostrar solo «{label}» ({keys})"),
    chip_none: |label| format!("Ningún archivo con el estado «{label}»"),

    cancelled: "Cancelado. No se escribió nada.",
    run_died: "La ejecución terminó inesperadamente.",
    run_not_started: |why| {
        format!("No se pudo iniciar la ejecución ({why}). Reinicie Veritas y vuelva a intentarlo.")
    },
    reused_read: |reused, read| {
        format!(
            "Reutilizadas: {}. Leídos: {}.",
            checksums(reused),
            files(read)
        )
    },
    unreadable_files: |count| {
        format!(
            "{} (estado «error»).",
            plural(count, "archivo no legible", "archivos no legibles")
        )
    },
    written: |md5, count| format!("{md5} escrito: {}.", files(count)),
    replaced: |md5, count| format!("{md5} reemplazado: {}.", files(count)),
    unchanged_since: "No ha cambiado nada respecto al archivo anterior.",
    changed_since: |parts| format!("Respecto al archivo anterior: {parts}."),
    damage_kept: "El archivo nuevo ha asumido también el estado posiblemente dañado; el \
                  historial registra las sumas de verificación anteriores.",
    verified_one: |md5| format!("Verificado con {md5}: el archivo está ok."),
    verified_all: |md5, count| format!("Verificado con {md5}: todo ok ({}).", files(count)),
    verified_findings: |md5, parts| format!("Verificado con {md5}: {parts}."),
    title_writing: "Escribiendo",
    title_cancelling: "Cancelando",

    crash_notice: |log| {
        format!("Veritas se interrumpió la última vez por un error del programa. Informe: {log}")
    },
    drop_running: "Durante una ejecución no se puede soltar una carpeta. Pulse Esc dos veces para \
                   cancelar.",
    drop_many: |first| format!("Se soltaron varios elementos – solo cuenta el primero: «{first}»."),
    drop_file: "Archivo soltado – cuenta la carpeta que lo contiene.",
    folder_running: "Cambie de carpeta después de la ejecución. Pulse Esc dos veces para \
                     cancelar.",
    not_a_folder: |path| {
        format!(
            "«{path}» no es una carpeta (o ya no lo es). Elija otra en el árbol de la izquierda."
        )
    },
    folder_unreadable: "No se pudo leer la carpeta. Vuelva a elegirla en el árbol de la \
                        izquierda.",
    recent_gone: |path| format!("«{path}» ya no existe; se ha quitado de las carpetas recientes."),
    no_findings: "No hay hallazgos en la lista.",
    open_gone: |path| format!("{path} ya no está en la carpeta y no se puede abrir."),
    open_refused: |path, why| format!("{path} no se puede abrir: {why}."),
    open_failed: |path, why| {
        format!(
            "{path} no se puede abrir ({why}). ¿Hay un programa asociado a este tipo de \
             archivo? «Mostrar en la carpeta» del menú contextual puede ayudar."
        )
    },
    open_json_failed: |path, why| {
        format!(
            "{path} no se puede abrir ({why}). Cualquier editor o navegador también muestra el \
             archivo."
        )
    },
    delete_running: "Durante una ejecución no se elimina nada. Pulse Esc dos veces para cancelar.",
    delete_own: "Veritas no elimina el archivo de sumas de verificación ni el historial.",
    delete_refused: |path, why| format!("{path} {why}; Veritas no lo elimina."),
    delete_gone: |path| format!("{path} ya no está en la carpeta."),
    kept: |path| format!("{path} se conserva."),
    trash_failed: |path, why| {
        format!(
            "{path} no se pudo mover a la Papelera ({why}). ¿Sigue abierto en algún programa? \
             Entonces cierre el programa y vuelva a intentarlo."
        )
    },
    trashed: |path| format!("{path} está en la Papelera."),
    reveal_gone: |path| format!("{path} ya no está; no hay nada que mostrar."),
    reveal_refused: |path, why| format!("No se puede mostrar la carpeta de {path}: {why}."),
    reveal_failed: |path, why| format!("No se puede mostrar la carpeta de {path} ({why})."),
    md5_copied: |algo, path| format!("Se copió el {algo} de {path}."),
    close_esc: "Cerrar (Esc)",
    close: "Cerrar",

    replace_title: |md5| format!("¿Reemplazar {md5}?"),
    replace: "Reemplazar",
    keep: "Conservar",
    replace_body: |md5, count, history, advise| {
        let advice = if advise {
            " Si tiene dudas, verifique antes (F5)."
        } else {
            ""
        };
        format!(
            "{md5} contiene {}. «Reemplazar» vuelve a escribir el archivo con el estado actual \
             de la carpeta. A partir de ahí, ese estado se da por correcto, incluso para los \
             archivos que hayan cambiado sin que nadie lo notara.{advice} El historial en \
             {history} registra qué sumas de verificación cambian; no se crea ninguna copia de \
             seguridad (.md5.bak).",
            checksums(count)
        )
    },
    replace_reuse: |count, verify, at| {
        let from = if verify {
            "la verificación"
        } else {
            "la creación del archivo"
        };
        format!(
            "Reutiliza hasta {} de {from} ({at}); solo se leen los archivos que son nuevos \
             desde entonces, tienen otro tamaño u otra fecha de modificación o no se podían \
             leer.",
            checksums(count)
        )
    },
    replace_nested: |count| {
        let what = if count == 1 {
            "1 entrada de las subcarpetas se queda como está".to_string()
        } else {
            format!(
                "{} entradas de las subcarpetas se quedan como están",
                n(count)
            )
        };
        format!("{what}, porque «Subcarpetas» está desactivado.")
    },
    close_writing_body: |folder, md5| {
        format!(
            "Veritas está escribiendo el resultado de «{folder}» (el historial y, al crear, \
             también {md5}). Al cerrar, la ventana espera hasta que termine."
        )
    },
    close_title: "¿Cancelar la ejecución y cerrar?",
    close_body: |folder, md5| {
        format!(
            "Veritas está calculando los hashes de «{folder}». Al cerrar, la ejecución se \
             cancela; no se escribe nada – ni {md5} ni el historial."
        )
    },
    cancel_close: "Cancelar y cerrar",
    keep_running: "Continuar",
    trash_title: |name| format!("¿Mover {name} a la Papelera?"),
    to_trash: "Mover a la Papelera",
    trash_body: |path| format!("{path} se mueve a la Papelera y se puede restaurar desde allí."),
    trash_body_recorded: |md5| {
        format!(
            "La suma de verificación se queda en {md5}: el archivo figurará como «falta» en la \
             lista hasta que «Crear MD5» lo quite. El historial también lo registrará."
        )
    },
    enter_tip: |label| format!("{label} (Intro)"),
    esc_tip: |label| format!("{label} (Esc)"),

    cancelling_lead: "Cancelando – no se escribe nada…",
    armed_lead: "Pulse Esc otra vez para cancelar – no se escribe nada",
    cancel: "Cancelar",
    cancel_tip: "Cancelar (Esc dos veces). No se escribe nada.",
    cancel_late: "Ya no se puede cancelar.",
    files_progress: |done, total| format!("{done} de {total} archivos"),
    reused_progress: |count| {
        if count == 1 {
            "1 suma de verificación reutilizada".to_string()
        } else {
            format!("{} sumas de verificación reutilizadas", n(count))
        }
    },
    left: |eta| format!("quedan {eta}"),

    tree_running: "Cambie de carpeta después de la ejecución (Esc dos veces cancela).",

    list_reading: "Leyendo…",
    no_files_sub_on: "No hay archivos directamente en esta carpeta. «Crear MD5» incluye las \
                      subcarpetas.",
    no_files_sub_off: "No hay archivos directamente en esta carpeta. Para incluir las \
                       subcarpetas, active «Subcarpetas» en la barra de herramientas.",
    no_files: "No hay archivos en esta carpeta.",
    no_hits: |query| format!("Sin resultados para «{query}»."),
    no_status: "No hay archivos con este estado.",
    col_status: "Estado",
    col_name: "Nombre",
    col_size: "Tamaño",
    col_modified: "Modificado",
    mark_open: "pendiente",
    mark_ok: "ok",
    mark_wrong: "distinto",
    mark_damaged: "dañado",
    mark_deleted: "falta",
    mark_new: "nuevo",
    mark_error: "error",
    mark_source: "md5",
    mark_report: "historial",
    change_changed: "modificado",
    change_added: "añadido",
    change_dropped: "quitado",
    counted,
    history_tip: "Mostrar todas las entradas de este archivo (Ctrl+H)",
    menu_history: "Mostrar historial",
    menu_open: "Abrir",
    menu_file_history: "Historial del archivo",
    menu_reveal: "Mostrar en la carpeta",
    menu_copy: |algo| format!("Copiar {algo}"),
    menu_trash: "Mover a la Papelera …",
    why_gone: "El archivo ya no está.",
    why_no_history: "Esta carpeta aún no tiene historial.",
    why_no_md5: |algo| format!("Este archivo aún no tiene {algo}. «Verificar» (F5) lo calcula."),

    empty_title: "Elija una carpeta",
    empty_text: "Elija una carpeta en el árbol de la izquierda o arrastre una carpeta a la \
                 ventana.",
    recent_title: "Carpetas recientes",
    open_recent: |path, key| format!("Abrir {path} ({key})"),
    keys_title: "Teclas",
    empty_keys: [
        ("Ctrl+Intro", "Crear MD5"),
        ("F5", "Verificar"),
        ("Ctrl+H", "Historial: qué cambió y cuándo"),
        ("Supr", "Archivo a la Papelera, con confirmación"),
        ("Esc", "un paso atrás; en ejecución, dos veces: cancelar"),
    ],

    about_title: "Acerca de Veritas",
    about_close: "Esc / F1 cierra",
    about_text: "Veritas verifica archivos y carpetas con MD5 e indica si un archivo multimedia \
                 que ya no coincide sigue siendo estructuralmente íntegro – en local, sin \
                 cuenta ni nube. Software libre: el código fuente es abierto.",
    about_version: "Versión",
    about_copyright: "Copyright",
    about_license: "Licencia",
    about_source: "Código fuente",
    report_title: "Informar de un error",
    report_text: "¿Algo no funciona? Descríbalo en un issue de GitHub; hace falta una cuenta \
                  gratuita de GitHub. La versión y el sistema ya vienen rellenados; no hacen \
                  falta nombres de archivo ni rutas.",
    report_link: "Informar de un error en GitHub",
    wish_title: "Ideas",
    wish_text: "¿Una idea que haga más fácil o más seguro verificar con Veritas? Escríbala como \
                propuesta, también con una cuenta de GitHub.",
    wish_link: "Proponer una idea en GitHub",
    third_title: "Terceros",
    third_text: "Veritas usa bibliotecas y tipografías libres de otros: Noto Sans y DejaVu Sans \
                 Mono (Linux), los iconos de Lucide y las tipografías de respaldo de egui \
                 (Windows). Sus textos de licencia acompañan al programa.",
    third_link: "Todos los terceros y licencias",

    history_title: "Historial",
    tab_runs: "Ejecuciones",
    tab_file: |name| format!("Archivo: {name}"),
    action_created: "MD5 creado",
    action_replaced: "MD5 reemplazado",
    action_verified: "Verificado",
    short_created: "creado",
    short_replaced: "reemplazado",
    short_verified: "verificado",
    run_cancelled: "cancelada",
    no_change: "sin cambios",
    all_ok: "todo ok",
    first_facts: |count| {
        format!(
            "{} en el primer archivo de sumas de verificación.",
            files(count)
        )
    },
    unchanged_count: |count| format!("{count} sin cambios"),
    flag_damaged: "Hallazgo: dañado o ilegible",
    flag_differs: "Hallazgo: distinto o falta",
    baseline_note: "Con el primer archivo de sumas de verificación, el historial no registra \
                    archivos sueltos. Todo archivo que más adelante no aparezca como \
                    modificado conserva este estado.",
    generate_none: "No ha cambiado nada respecto al .md5 anterior.",
    verify_none: "Todos los archivos coinciden con el archivo de sumas de verificación.",
    before: "antes",
    after: "después",
    entry_tip: "Un clic muestra todas las entradas de este archivo.",
    file_none: |since| {
        format!(
            "Ninguna entrada desde el {since}: ninguna ejecución ha notificado que este archivo \
             sea distinto o nuevo, que falte o que no se pueda leer."
        )
    },
    file_events: |count, since| {
        format!(
            "{} desde el {since}, la más reciente arriba.",
            entries(count)
        )
    },
    show_run: "Mostrar esta ejecución",
    open_json: "Abrir como JSON",
    open_json_tip: |name| format!("Abrir {name} en el programa que el sistema usa para JSON"),
    keys_file: "← / → pestaña  ·  clic en una fecha muestra la ejecución  ·  Esc cierra",
    keys_runs_file: "↑ / ↓ ejecución  ·  ← / → pestaña  ·  Esc cierra",
    keys_runs: "↑ / ↓ ejecución  ·  clic en un archivo muestra sus entradas  ·  Esc cierra",
    hint_none: |since| format!("Historial: sin cambios desde el {since}"),
    hint_last: |what, day| format!("Historial: {what} · {day}"),
    hint_last_many: |what, day, count| format!("Historial: {what} · {day} ({})", entries(count)),

    sys_not_found: "no encontrado",
    sys_denied: "sin permiso",
    sys_message: |text| format!("mensaje del sistema: {text}"),
    no_trash_unc: "Las rutas de red no tienen Papelera; Veritas no elimina allí.",
    no_trash_network: "Las unidades de red no tienen Papelera; Veritas no elimina allí.",
    no_trash_removable: "Las unidades extraíbles no tienen Papelera; Veritas no elimina allí.",

    msg,
};

/// Texts from run, preview and structure check (`Msg`, placeholders as in the catalogue).
fn msg(msg: Msg) -> &'static str {
    match msg {
        Msg::PhaseReady => "Listo",
        Msg::PhaseReadMd5 => "Leyendo el archivo de sumas de verificación…",
        Msg::PhaseWalk => "Leyendo la carpeta…",
        Msg::PhaseMatch => "Comparando las sumas de verificación conocidas…",
        Msg::PhaseHash => "Calculando las sumas de verificación…",
        Msg::PhaseIntegrity => "Verificando la integridad…",
        Msg::PhaseWrite => "Escribiendo los archivos…",
        Msg::PhaseDone => "Terminado",
        Msg::NoFolder => "Primero elija una carpeta a la izquierda.",
        Msg::NotInFolder => "no está en la carpeta elegida",
        Msg::IsLink => "es un enlace o está detrás de uno; Veritas no sigue los enlaces",
        Msg::NotRegularFile => "no es un archivo normal",
        Msg::NotAFileInFolder => "no es un archivo de la carpeta",
        Msg::PreviewNotUtf8 => "{} no está en UTF-8; se leyó como {}.",
        Msg::Unreadable => "{} no se puede leer: {}",
        Msg::NoHistory => "Aún no hay historial. «Crear MD5» o «Verificar» crea {}.",
        Msg::NotOurHistory => "{} no es un historial de Veritas.",
        Msg::HistoryEmpty => "{} aún no contiene ninguna ejecución.",
        Msg::TooLarge => {
            "{} es demasiado grande ({n}\u{a0}MiB); Veritas lee como máximo {n}\u{a0}MiB. ¿Es el archivo correcto?"
        }
        Msg::FileReadDenied => "Sin permiso de lectura",
        Msg::FileVanished => "El archivo desapareció durante la lectura",
        Msg::FileReadError => "Error de lectura: {m}",
        Msg::NotRegular => "No es un archivo normal: {}",
        Msg::SymlinkLoop => "Bucle de enlaces simbólicos: {m}",
        Msg::DirDenied => "Sin permiso: {m}",
        Msg::DirReadError => "Error al leer la carpeta: {m}",
        Msg::UnknownPlace => "(desconocido)",
        Msg::BadName => "El nombre de archivo no es UTF-8 válido o está fuera de la carpeta",
        Msg::NotChecked => "No verificado – {m}",
        Msg::RunCancelledRow => "Ejecución cancelada, sin comparar.",
        Msg::NotADirectory => "No es una carpeta: {}",
        Msg::ReadDeniedAt => "Sin permiso de lectura: {} ({})",
        Msg::ReadDeniedPlain => "Sin permiso de lectura: {}",
        Msg::NotFoundAt => "No encontrado: {} ({})",
        Msg::NotFoundPlain => "No encontrado: {}",
        Msg::IoErrorAt => "Error de entrada/salida: {} ({})",
        Msg::IoErrorPlain => "Error de entrada/salida: {}",
        Msg::WriteDenied => "Sin permiso de escritura: {} ({})",
        Msg::WriteReadOnly => "La unidad está protegida contra escritura: {} ({})",
        Msg::WriteFull => "La unidad está llena: {} ({})",
        Msg::WriteInTheWay => "Hay un archivo temporal que lo impide: {} ({})",
        Msg::WriteFailed => "Error de escritura: {} ({})",
        Msg::ChecksumMissing => {
            "No se encontró el archivo de sumas de verificación: {}. Créelo con «Crear MD5»."
        }
        Msg::JsonFailed => "No se pudo crear el JSON: {}",
        Msg::RunCancelled => "Ejecución cancelada",
        Msg::ThreadPool => "No se pudo iniciar el grupo de subprocesos: {}",
        Msg::RenameFailed => {
            "{m}. El historial ya registra esta ejecución; el archivo de sumas de verificación quedó sin cambios."
        }
        Msg::ReuseFromVerify => {
            "Reutilizadas de la última verificación (tamaño y fecha de modificación sin cambios): {p}. Leídos: {p}."
        }
        Msg::ReuseFromGenerate => {
            "Reutilizadas de la última creación (tamaño y fecha de modificación sin cambios): {p}. Leídos: {p}."
        }
        Msg::KnownStale => {
            "Las sumas de verificación de la última ejecución ya no sirven (cambió el archivo de sumas o los ajustes); se leyeron todos los archivos."
        }
        Msg::NestedKept => {
            "Entradas de subcarpetas mantenidas sin cambios: {n} (Subcarpetas está desactivado)."
        }
        Msg::NestedSkipped => {
            "Entradas de subcarpetas sin verificar: {n} (Subcarpetas está desactivado)."
        }
        Msg::BelowUnreadableKept => {
            "Entradas en carpetas que no se pueden leer (sin verificar, mantenidas sin cambios): {n}."
        }
        Msg::BelowUnreadable => {
            "Entradas en carpetas que no se pueden leer (sin verificar): {n}."
        }
        Msg::AbortedIntegrity => {
            "Cancelado durante la verificación de integridad. No se escribió nada."
        }
        Msg::AlgoMissing => "Falta {}; este algoritmo no se compara.",
        Msg::ReplacingInvalid => "Se reemplaza el archivo de sumas de verificación anterior: {m}",
        Msg::ChecksumInvalid => "Archivo de sumas de verificación no válido ({}): {m}",
        Msg::NotUtf8Read => "{} no está en UTF-8 y se leyó como {}.",
        Msg::Line => "línea {n}: {m}",
        Msg::LineWarning => "{}: línea {n}: {m}",
        Msg::HashLenMismatch => "{}: la longitud del hash no corresponde a {}: {}",
        Msg::SymlinksSkipped => {
            "Enlaces simbólicos omitidos: {n} (Veritas no sigue los enlaces)."
        }
        Msg::SpecialSkipped => "Archivos especiales omitidos: {n} (no son archivos normales).",
        Msg::NothingWritten => "{m} No se escribió nada.",
        Msg::NotInHistory => "{m} Esta ejecución no consta en el historial.",
        Msg::HistoryUnreadable => "No se puede leer el historial {} ({}); se deja sin cambios.",
        Msg::HistoryOtherFormat => {
            "El historial {} tiene el formato «{}» de otra versión de Veritas; esta versión no lo sobrescribe."
        }
        Msg::HistoryReplaced => "{} no era un historial de Veritas y se reemplazó.",
        Msg::DuplicatePath => "la ruta {} aparece más de una vez; cuenta el último hash",
        Msg::HashLength => "el hash debe tener 32 (MD5) o 64 (SHA-256) dígitos hexadecimales",
        Msg::HashLengthAny => {
            "el hash debe tener 32 (MD5), 40 (SHA-1), 64 (SHA-256) o 128 (SHA-512) dígitos hexadecimales"
        }
        Msg::TagLength => "una línea en formato BSD necesita «(archivo) = hash» con un hash {}",
        Msg::NoSeparator => "tras el hash faltan dos espacios o una marca binaria",
        Msg::PathMissing => "falta la ruta",
        Msg::PathOutside => "la ruta no debe salir de la carpeta",
        Msg::PathNul => "la ruta contiene un carácter nulo",
        Msg::PathRelative => "la ruta debe ser relativa",
        Msg::PathDrive => "la ruta debe ser relativa (unidad o dos puntos)",
        Msg::ReadDenied => "Archivo ilegible: sin permiso",
        Msg::ReadFailed => "Archivo ilegible: {}",
        Msg::Empty => "El archivo está vacío",
        Msg::HeadUnreadableAny => "No se puede leer el encabezado del archivo",
        Msg::UnknownFormat => "Formato de archivo desconocido, sin verificación de estructura",
        Msg::TooShort => "{}: archivo demasiado corto",
        Msg::HeadUnreadable => "{}: no se puede leer el encabezado del archivo",
        Msg::TailUnreadable => "{}: no se puede leer el final del archivo",
        Msg::JpegNoSoi => "JPEG: falta SOI (FF D8)",
        Msg::JpegOk => "JPEG: SOI (FF D8) y EOI (FF D9) presentes",
        Msg::JpegOkTrailing => {
            "JPEG: SOI (FF D8) y EOI (FF D9) presentes; bytes de datos adicionales después: {n}"
        }
        Msg::JpegNoEoi => "JPEG: falta EOI (FF D9) al final del archivo",
        Msg::JpegCut => "JPEG: el archivo termina antes de los datos de imagen",
        Msg::PngNoSignature => "PNG: falta la firma",
        Msg::PngNoIend => "PNG: falta el chunk IEND al final del archivo",
        Msg::PngChunkUnreadable => "PNG: chunk ilegible",
        Msg::PngChunkPastEnd => "PNG: un chunk sobrepasa el final del archivo, falta IEND",
        Msg::PngOk => "PNG: firma y chunk IEND presentes",
        Msg::PngOkTrailing => "PNG: firma y chunk IEND presentes; bytes añadidos después: {n}",
        Msg::PngManyChunks => "PNG: muchísimos chunks, final sin verificar",
        Msg::RiffNoSignature => "{}: falta la firma RIFF",
        Msg::RiffTooLarge => "{}: el tamaño RIFF excede el final del archivo",
        Msg::RiffOk => "{}: firma RIFF presente",
        Msg::HeifNoFtyp => "HEIF/HEIC: falta la caja ftyp",
        Msg::HeifFtypShort => "HEIF/HEIC: caja ftyp incompleta",
        Msg::HeifBrand => "HEIF/HEIC: ftyp con la marca {}",
        Msg::HeifNoBrand => "HEIF/HEIC: ftyp sin marca heic/heif/mif1/msf1",
        Msg::TiffShort => "{}: archivo más corto que el encabezado",
        Msg::TiffNoSignature => "TIFF: falta la firma II* o MM*",
        Msg::TiffIfdOutside => "{}: el desplazamiento IFD queda fuera del archivo",
        Msg::TiffLittle => "TIFF: encabezado little-endian y desplazamiento IFD válidos",
        Msg::TiffBig => "TIFF: encabezado big-endian y desplazamiento IFD válidos",
        Msg::BigTiffBad => "BigTIFF: encabezado no válido (tamaño de desplazamiento distinto de 8)",
        Msg::BigTiffOk => "BigTIFF: encabezado y desplazamiento IFD válidos",
        Msg::Mp4AtomCut => "MP4/MOV: el átomo {} sobrepasa el final del archivo (truncado)",
        Msg::Mp4Trailing => "MP4/MOV: átomos completos; bytes añadidos después: {n}",
        Msg::Mp4CutBeforeMoov => {
            "MP4/MOV: ftyp presente, estructura de átomos truncada antes de moov"
        }
        Msg::Mp4AtomTooLarge => "MP4/MOV: el tamaño del átomo excede el final del archivo",
        Msg::Mp4HeaderUnreadable => "MP4/MOV: no se puede leer el encabezado del átomo",
        Msg::Mp4ManyAtoms => "MP4/MOV: moov presente, muchísimos átomos, final sin verificar",
        Msg::Mp4Ok => "MP4/MOV: ftyp y moov presentes",
        Msg::Mp4QuickTime => "MP4/MOV: moov presente (QuickTime sin ftyp)",
        Msg::Mp4NoFtyp => "MP4/MOV: falta la caja ftyp",
        Msg::Mp4NoMoov => "MP4/MOV: caja moov no encontrada en el rango examinado",
        Msg::MkvNoHeader => "MKV: falta el encabezado EBML 1A 45 DF A3",
        Msg::MkvPastEnd => "MKV: el encabezado EBML sobrepasa el final del archivo",
        Msg::MkvShort => "MKV: encabezado EBML incompleto",
        Msg::MkvOk => "MKV: encabezado EBML presente",
        Msg::MkvLength => "MKV: no se puede leer la longitud EBML",
        Msg::PdfNoHead => "PDF: falta el encabezado %PDF-",
        Msg::PdfOk => "PDF: encabezado %PDF- y %%EOF en la última ventana (2048\u{a0}bytes)",
        Msg::PdfNoEof => "PDF: falta %%EOF en la última ventana (2048\u{a0}bytes)",
        Msg::OfficeShort => "Archivo de Office demasiado corto para un ZIP",
        Msg::OfficeUnreadable => "Archivo de Office ilegible",
        Msg::OfficeOle => "Archivo de Office cifrado o en formato OLE, estructura sin verificar",
        Msg::OfficeNoZip => "El archivo de Office no empieza con un encabezado local ZIP",
        Msg::ZipNoEocd => "Falta el fin del directorio central ZIP (EOCD)",
        Msg::ZipEocdUnreadable => "No se puede leer el fin del directorio central ZIP",
        Msg::ZipDirOutside => "El directorio central ZIP queda fuera del archivo",
        Msg::ZipDirHuge => "Directorio central ZIP muy grande, sin verificar",
        Msg::OoxmlOk => "Office Open XML: estructura ZIP y [Content_Types].xml presentes",
        Msg::NoContentTypes => "Falta [Content_Types].xml",
        Msg::ZipManyEntries => "ZIP con muchísimas entradas, no verificado por completo",
        Msg::ZipDirUnreadableWhy => "No se puede leer el directorio central ZIP: {}",
        Msg::ZipDirUnreadable => "No se puede leer el directorio central ZIP",
        Msg::ZipDirBroken => "Directorio central ZIP dañado",
        Msg::ZipNameUnreadable => "No se puede leer un nombre de archivo ZIP",
        Msg::ZipLocalMissing => "Falta el encabezado local ZIP de [Content_Types].xml",
        Msg::ZipLocalUnreadable => "No se puede leer el encabezado local ZIP",
        Msg::ZipLocalBad => "El encabezado local ZIP de [Content_Types].xml no es válido",
        Msg::RunCancelledStructure => "Ejecución cancelada, estructura sin verificar",
    }
}
