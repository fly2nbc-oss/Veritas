//! Italian.
//!
//! Core terms, uniform across all texts: Verifica (Prüfen), Crea MD5 (MD5 erzeugen),
//! checksum (Prüfsumme, invariable), cronologia (Verlauf), esecuzione (Lauf), Cestino
//! (Papierkorb), sottocartelle (Unterordner). Form of address: commands on buttons in the
//! imperative (Verifica, Apri), instructions in a sentence in the infinitive (Premere,
//! Scegliere), never "tu" in running text. Quotation marks « ».
//!
//! The status labels (`mark_*`, `change_*`) are singular: they are the word for one file.
//! Where things are counted (`count_parts_in`, `history::facts`), `counted` applies with the
//! plural ("3 diversi"). In the templates of `msg`, colon forms avoid agreement
//! ("Voci … : {n}"); `reused`, `read_files` and `unreadable` know the singular.
//! The seven chips of the status bar have only a good 279 px for their words at 880 px (test
//! `fixed_labels_fit_in_every_language`): hence `corrotto` instead of `danneggiato`.

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
    plural(count, "file", "file")
}

fn checksums(count: usize) -> String {
    plural(count, "checksum", "checksum")
}

fn entries(count: usize) -> String {
    plural(count, "voce", "voci")
}

/// "1 diverso", "2 diversi", "2 errori". The words agree with « file » (masculine),
/// « errore » is the noun itself.
fn counted(change: Change, count: usize) -> String {
    let (one, many) = match change {
        Change::Differs => ("diverso", "diversi"),
        Change::Damaged => ("corrotto", "corrotti"),
        Change::Changed => ("aggiornato", "aggiornati"),
        Change::New => ("nuovo", "nuovi"),
        Change::Added => ("aggiunto", "aggiunti"),
        Change::Missing => ("mancante", "mancanti"),
        Change::Dropped => ("rimosso", "rimossi"),
        Change::Unreadable => ("errore", "errori"),
    };
    plural(count, one, many)
}

/// "1 checksum riutilizzato", "3 checksum riutilizzati".
fn reused(count: usize) -> String {
    plural(count, "checksum riutilizzato", "checksum riutilizzati")
}

/// "1 file letto", "3 file letti".
fn read_files(count: usize) -> String {
    plural(count, "file letto", "file letti")
}

/// "1 file non leggibile", "3 file non leggibili".
fn unreadable(count: usize) -> String {
    plural(count, "file non leggibile", "file non leggibili")
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
    runs: |count| plural(count, "esecuzione", "esecuzioni"),

    key_ctrl: "Ctrl",
    key_enter: "Invio",
    key_delete: "Canc",

    about_tip: "Informazioni su Veritas (F1)",
    language_tip: |name| format!("Lingua: {name} (Ctrl+L)"),
    search_hint: "Cerca",
    search_tip: "Filtra i nomi dei file (Ctrl+F)",
    threads_tip: "Thread: file di cui si calcola l’hash contemporaneamente (Ctrl+,; ↑/↓ per \
                  cambiare). Su un disco rigido (HDD) sempre uno solo, perché lì è più veloce. \
                  Vale dalla prossima esecuzione.",
    threads_hdd: "Questa cartella si trova su un disco rigido (HDD): Veritas calcola qui l’hash \
                  di un file dopo l’altro, quindi il numero non vale.",
    locked: "Bloccato durante un’esecuzione. Premere Esc due volte per annullare.",
    subfolders_on: "Sottocartelle: incluse. Verifica e Crea MD5 comprendono anche i file nelle \
                    sottocartelle. Clic: solo i file direttamente nella cartella.",
    subfolders_off: "Sottocartelle: escluse. Verifica e Crea MD5 comprendono solo i file \
                     direttamente nella cartella; le voci nelle sottocartelle restano \
                     invariate. Clic: includere le sottocartelle.",
    theme_light: "Tema: chiaro",
    theme_dark: "Tema: scuro",
    theme_system: "Tema: come il sistema",
    verify: "Verifica",
    verify_tip: |md5| format!("Verifica la cartella rispetto a {md5} (F5)"),
    generate: "Crea MD5",
    generate_tip: |md5| {
        format!("Scrive {md5} con i checksum di tutti i file della cartella (Ctrl+Invio)")
    },
    md5_here: "File di checksum di questa cartella",
    md5_missing: "Non è ancora nella cartella. «Crea MD5» lo crea.",

    loading: "La cartella è ancora in lettura. Riprovare tra un attimo.",
    writing: "Scrittura in corso – quasi finito.",
    unread: "Impossibile leggere la cronologia. Scegliere di nuovo la cartella a sinistra.",
    run_busy: "Un’esecuzione è già in corso. Premere Esc due volte per annullarla.",
    verify_needs_md5: |md5| format!("Nella cartella non c’è ancora {md5}. Prima «Crea MD5»."),

    pick_folder: "Scegliere una cartella nell’albero.",
    reading_folder: |folder| format!("Lettura di «{folder}»…"),
    folder_unreadable_status: "Impossibile leggere la cartella.",
    folder_status: |md5, count, exists| {
        if exists {
            format!("{md5} · {}", files(count))
        } else {
            format!("Ancora nessun {md5} · {}", files(count))
        }
    },
    md5_unchanged: |md5| format!("{md5} resta invariato."),
    status_generating: |md5| format!("Scrittura di {md5}…"),
    status_verifying: |md5| format!("Verifica rispetto a {md5}…"),
    cancelling: "Annullamento…",
    row_history: "tutte le esecuzioni di questa cartella; doppio clic o Ctrl+H le mostra",
    row_md5: "il doppio clic lo apre",
    md5_then_now: |algo, old, new| format!("{algo} salvato {old}, ora {new}"),
    md5_then_now_lines: |algo, old, new| format!("{algo} salvato {old}\n{algo} ora     {new}"),
    short_damaged: "contenuto modificato, struttura forse danneggiata",
    short_intact: "contenuto modificato; inizio e fine in ordine – danni all’interno non esclusi",
    short_unchecked: "contenuto modificato; struttura di questo tipo di file non verificata",
    detail_changed_intact: "Il contenuto è cambiato. Inizio e fine sono in ordine – questo non \
                            esclude un danno all’interno.",
    detail_changed_unchecked: "Il contenuto è cambiato. Veritas non verifica la struttura di \
                               questo tipo di file.",
    detail_damaged: "Il contenuto è cambiato e la struttura è forse danneggiata.",
    restore_help: "Se non è stato modificato di proposito, ripristinarlo dal backup.",
    structure: |reason| format!("Verifica della struttura: {reason}"),
    detail_deleted: |md5| {
        format!(
            "Figura in {md5}, ma non è più nella cartella. Eliminato di proposito? Allora \
             «Crea MD5» lo rimuove."
        )
    },
    detail_new: |md5| format!("È nella cartella, ma non figura in {md5}. «Crea MD5» lo aggiunge."),
    detail_open: |md5| format!("Figura in {md5}; non ancora verificato. «Verifica» (F5) lo legge."),
    detail_ok: |md5| format!("Corrisponde a {md5}."),
    detail_error: "È aperto o bloccato in un altro programma? Allora chiuderlo e verificare di \
                   nuovo (F5).",

    chip_all: |keys| format!("Mostra tutti i file (Esc, {keys})"),
    chip_only: |label, keys| format!("Mostra solo «{label}» ({keys})"),
    chip_none: |label| format!("Nessun file con stato «{label}»"),

    cancelled: "Annullato. Non è stato scritto nulla.",
    run_died: "L’esecuzione si è interrotta in modo imprevisto.",
    run_not_started: |why| {
        format!("Impossibile avviare l’esecuzione ({why}). Riavviare Veritas e riprovare.")
    },
    reused_read: |reused_count, read| format!("{}, {}.", reused(reused_count), read_files(read)),
    unreadable_files: |count| format!("{} (stato «errore»).", unreadable(count)),
    written: |md5, count| format!("{md5} scritto: {}.", files(count)),
    replaced: |md5, count| format!("{md5} sostituito: {}.", files(count)),
    unchanged_since: "Rispetto al file precedente non è cambiato nulla.",
    changed_since: |parts| format!("Rispetto al file precedente: {parts}."),
    damage_kept: "Il nuovo file ha ripreso anche lo stato forse danneggiato; la cronologia \
                  conserva i vecchi checksum.",
    verified_one: |md5| format!("Verifica rispetto a {md5}: il file è ok."),
    verified_all: |md5, count| {
        if count == 1 {
            format!("Verifica rispetto a {md5}: 1 file ok.")
        } else {
            format!("Verifica rispetto a {md5}: tutti i {} ok.", files(count))
        }
    },
    verified_findings: |md5, parts| format!("Verifica rispetto a {md5}: {parts}."),
    title_writing: "Scrittura in corso",
    title_cancelling: "Annullamento in corso",

    crash_notice: |log| {
        format!(
            "Veritas si è interrotto l’ultima volta per un errore del programma. Rapporto: {log}"
        )
    },
    drop_running: "Durante un’esecuzione non si può rilasciare una cartella. Premere Esc due \
                   volte per annullare.",
    drop_many: |first| format!("Rilasciati più elementi – conta solo il primo: «{first}»."),
    drop_file: "File rilasciato – conta la cartella che lo contiene.",
    folder_running: "Cambiare cartella solo dopo l’esecuzione. Premere Esc due volte per \
                     annullare.",
    not_a_folder: |path| {
        format!("«{path}» non è (più) una cartella. Sceglierne un’altra nell’albero a sinistra.")
    },
    folder_unreadable: "Impossibile leggere la cartella. Sceglierla di nuovo nell’albero a \
                        sinistra.",
    recent_gone: |path| format!("«{path}» non esiste più; è stata rimossa dalle cartelle recenti."),
    no_findings: "Nessuna anomalia nell’elenco.",
    open_gone: |path| format!("{path} manca e non può essere aperto."),
    open_refused: |path, why| format!("{path} non può essere aperto: {why}."),
    open_failed: |path, why| {
        format!(
            "{path} non può essere aperto ({why}). È configurato un programma per questo tipo \
             di file? «Mostra nella cartella» nel menu contestuale può aiutare."
        )
    },
    open_json_failed: |path, why| {
        format!(
            "{path} non può essere aperto ({why}). Qualsiasi editor o browser mostra comunque il \
             file."
        )
    },
    delete_running: "Durante un’esecuzione non viene eliminato nulla. Premere Esc due volte per \
                     annullare.",
    delete_own: "Veritas non elimina il file di checksum né la cronologia.",
    delete_refused: |path, why| format!("{path} {why}; Veritas non lo elimina."),
    delete_gone: |path| format!("{path} non è più nella cartella."),
    kept: |path| format!("{path} resta."),
    trash_failed: |path, why| {
        format!(
            "Impossibile spostare {path} nel Cestino ({why}). È ancora aperto in un programma? \
             Allora chiudere il programma e riprovare."
        )
    },
    trashed: |path| format!("{path} è nel Cestino."),
    reveal_gone: |path| format!("{path} manca; non c’è nulla da mostrare."),
    reveal_refused: |path, why| format!("Impossibile mostrare la cartella di {path}: {why}."),
    reveal_failed: |path, why| format!("Impossibile mostrare la cartella di {path} ({why})."),
    md5_copied: |algo, path| format!("{algo} di {path} copiato."),
    close_esc: "Chiudi (Esc)",
    close: "Chiudi",

    replace_title: |md5| format!("Sostituire {md5}?"),
    replace: "Sostituisci",
    keep: "Mantieni",
    replace_body: |md5, count, history, advise| {
        let advice = if advise {
            " In caso di dubbio, verificare prima (F5)."
        } else {
            ""
        };
        format!(
            "{md5} contiene {}. «Sostituisci» riscrive il file con lo stato attuale della \
             cartella. Da quel momento questo stato conta come corretto – anche per i file \
             cambiati senza che nessuno se ne accorgesse.{advice} La cronologia in {history} \
             registra quali checksum cambiano; non viene creata nessuna copia di backup \
             (.md5.bak).",
            checksums(count)
        )
    },
    replace_reuse: |count, verify, at| {
        let from = if verify {
            "dalla verifica"
        } else {
            "dalla creazione del file"
        };
        format!(
            "Riutilizza fino a {} {from} delle {at}; vengono letti solo i file nuovi da allora, \
             con dimensione o data di modifica diversa o che non erano leggibili.",
            checksums(count)
        )
    },
    replace_nested: |count| {
        if count == 1 {
            "1 voce nelle sottocartelle resta invariata, perché «Sottocartelle» è disattivato."
                .to_owned()
        } else {
            format!(
                "{} nelle sottocartelle restano invariate, perché «Sottocartelle» è \
                 disattivato.",
                entries(count)
            )
        }
    },
    close_writing_body: |folder, md5| {
        format!(
            "Veritas sta scrivendo il risultato per «{folder}» (la cronologia e, con «Crea \
             MD5», anche {md5}). Alla chiusura la finestra attende la fine della scrittura."
        )
    },
    close_title: "Annullare l’esecuzione e chiudere?",
    close_body: |folder, md5| {
        format!(
            "Veritas sta elaborando «{folder}». Chiudendo, l’esecuzione viene annullata; non \
             viene scritto nulla – né {md5} né la cronologia."
        )
    },
    cancel_close: "Annulla e chiudi",
    keep_running: "Continua",
    trash_title: |name| format!("Spostare {name} nel Cestino?"),
    to_trash: "Sposta nel Cestino",
    trash_body: |path| {
        format!("{path} viene spostato nel Cestino e può essere ripristinato da lì.")
    },
    trash_body_recorded: |md5| {
        format!(
            "Il checksum resta in {md5}: il file compare poi come «mancante» nell’elenco finché \
             «Crea MD5» non lo rimuove. Lo registra anche la cronologia."
        )
    },
    enter_tip: |label| format!("{label} (Invio)"),
    esc_tip: |label| format!("{label} (Esc)"),

    cancelling_lead: "Annullamento – non viene scritto nulla…",
    armed_lead: "Premere di nuovo Esc per annullare – non viene scritto nulla",
    cancel: "Annulla",
    cancel_tip: "Annulla (Esc due volte). Non viene scritto nulla.",
    cancel_late: "Non è più possibile annullare.",
    files_progress: |done, total| format!("{done} di {total} file"),
    reused_progress: |count| reused(count),
    left: |eta| format!("ancora {eta}"),

    tree_running: "Cambiare cartella dopo l’esecuzione (Esc due volte annulla).",

    list_reading: "Lettura…",
    no_files_sub_on: "Nessun file direttamente in questa cartella. «Crea MD5» include le \
                      sottocartelle.",
    no_files_sub_off: "Nessun file direttamente in questa cartella. Per le sottocartelle, \
                       attivare «Sottocartelle» nella barra.",
    no_files: "Nessun file in questa cartella.",
    no_hits: |query| format!("Nessun risultato per «{query}»."),
    no_status: "Nessun file con questo stato.",
    col_status: "Stato",
    col_name: "Nome file",
    col_size: "Dimens.",
    col_modified: "Modificato",
    mark_open: "in attesa",
    mark_ok: "ok",
    mark_wrong: "diverso",
    mark_damaged: "corrotto",
    mark_deleted: "mancante",
    mark_new: "nuovo",
    mark_error: "errore",
    mark_source: "md5",
    mark_report: "cronologia",
    change_changed: "aggiornato",
    change_added: "aggiunto",
    change_dropped: "rimosso",
    counted,
    history_tip: "Mostra tutte le voci di questo file (Ctrl+H)",
    menu_history: "Mostra cronologia",
    menu_open: "Apri",
    menu_file_history: "Cronologia del file",
    menu_reveal: "Mostra nella cartella",
    menu_copy: |algo| format!("Copia {algo}"),
    menu_trash: "Sposta nel Cestino …",
    why_gone: "Il file manca.",
    why_no_history: "Per questa cartella non esiste ancora una cronologia.",
    why_no_md5: |algo| {
        format!("Per questo file non c’è ancora un {algo}. «Verifica» (F5) lo calcola.")
    },

    empty_title: "Scegliere una cartella",
    empty_text: "Scegliere una cartella nell’albero a sinistra o trascinarla nella finestra.",
    recent_title: "Aperte di recente",
    open_recent: |path, key| format!("Apri {path} ({key})"),
    keys_title: "Scorciatoie",
    empty_keys: [
        ("Ctrl+Invio", "Crea MD5"),
        ("F5", "Verifica"),
        ("Ctrl+H", "Cronologia: cosa è cambiato e quando"),
        ("Canc", "File nel Cestino, con conferma"),
        ("Esc", "Indietro; in esecuzione 2 volte: annulla"),
    ],

    about_title: "Informazioni su Veritas",
    about_close: "Esc / F1 chiude",
    about_text: "Veritas verifica file e cartelle con MD5 e segnala se un file multimediale non \
                 più corrispondente è ancora strutturalmente integro – in locale, senza account \
                 e senza cloud. Software libero: il codice sorgente è aperto.",
    about_version: "Versione",
    about_copyright: "Copyright",
    about_license: "Licenza",
    about_source: "Codice sorgente",
    report_title: "Segnala un problema",
    report_text: "Qualcosa non funziona? Descriverlo in una issue su GitHub – serve un account \
                  GitHub gratuito. Versione e sistema sono già compilati; nomi di file e \
                  percorsi non servono.",
    report_link: "Segnala un problema su GitHub",
    wish_title: "Idee",
    wish_text: "Un’idea che renda la verifica con Veritas più semplice o più sicura? Scriverla \
                come proposta – sempre con un account GitHub.",
    wish_link: "Proponi un’idea su GitHub",
    third_title: "Terze parti",
    third_text: "Veritas usa librerie e font liberi di altri: Noto Sans e DejaVu Sans Mono \
                 (Linux), le icone di Lucide e i font di riserva di egui (Windows). I loro testi \
                 di licenza sono inclusi nel programma.",
    third_link: "Tutte le terze parti e le licenze",

    history_title: "Cronologia",
    tab_runs: "Esecuzioni",
    tab_file: |name| format!("File: {name}"),
    action_created: "MD5 creato",
    action_replaced: "MD5 sostituito",
    action_verified: "Verificato",
    short_created: "creato",
    short_replaced: "sostituito",
    short_verified: "verificato",
    run_cancelled: "annullata",
    no_change: "nessuna modifica",
    all_ok: "tutto ok",
    first_facts: |count| format!("{} nel primo file di checksum.", files(count)),
    // Next to the counted statuses of the run list ("2 diversi"), again number before word.
    unchanged_count: |count| {
        let word = if count == "1" {
            "invariato"
        } else {
            "invariati"
        };
        format!("{count} {word}")
    },
    flag_damaged: "Esito: corrotto o non leggibile",
    flag_differs: "Esito: diverso o mancante",
    baseline_note: "Per il primo file di checksum la cronologia non registra i singoli file. \
                    Ogni file che in seguito non risulta modificato ha ancora questo stato.",
    generate_none: "Rispetto al .md5 precedente non è cambiato nulla.",
    verify_none: "Tutti i file corrispondono al file di checksum.",
    before: "prima",
    after: "dopo",
    entry_tip: "Un clic mostra tutte le voci di questo file.",
    file_none: |since| {
        format!(
            "Nessuna voce da {since}: nessuna esecuzione ha segnalato questo file come \
             diverso, nuovo, mancante o non leggibile."
        )
    },
    file_events: |count, since| format!("{} da {since}, la più recente in alto.", entries(count)),
    show_run: "Mostra questa esecuzione",
    open_json: "Apri come JSON",
    open_json_tip: |name| format!("Apri {name} nel programma che il sistema usa per JSON"),
    keys_file: "← / → scheda  ·  clic su una data mostra l’esecuzione  ·  Esc chiude",
    keys_runs_file: "↑ / ↓ esecuzione  ·  ← / → scheda  ·  Esc chiude",
    keys_runs: "↑ / ↓ esecuzione  ·  clic su un file mostra le sue voci  ·  Esc chiude",
    hint_none: |since| format!("Cronologia: nessuna modifica da {since}"),
    hint_last: |what, day| format!("Cronologia: {what} in data {day}"),
    hint_last_many: |what, day, count| {
        format!("Cronologia: {what} in data {day} ({} voci)", n(count))
    },

    sys_not_found: "non trovato",
    sys_denied: "nessun permesso",
    sys_message: |text| format!("messaggio del sistema: {text}"),
    no_trash_unc: "I percorsi di rete non hanno un Cestino; Veritas non elimina file lì.",
    no_trash_network: "Le unità di rete non hanno un Cestino; Veritas non elimina file lì.",
    no_trash_removable: "Le unità rimovibili non hanno un Cestino; Veritas non elimina file lì.",

    msg,
};

/// Texts from run, preview and structure check (`Msg`, placeholders as in the catalogue).
fn msg(msg: Msg) -> &'static str {
    match msg {
        Msg::PhaseReady => "Pronto",
        Msg::PhaseReadMd5 => "Lettura del file di checksum…",
        Msg::PhaseWalk => "Lettura della cartella…",
        Msg::PhaseMatch => "Confronto dei checksum noti…",
        Msg::PhaseHash => "Calcolo dei checksum…",
        Msg::PhaseIntegrity => "Verifica dell’integrità…",
        Msg::PhaseWrite => "Scrittura dei file…",
        Msg::PhaseDone => "Completato",
        Msg::NoFolder => "Scegliere prima una cartella a sinistra.",
        Msg::NotInFolder => "non si trova nella cartella scelta",
        Msg::IsLink => "è un collegamento o si trova dietro uno; Veritas non segue i collegamenti",
        Msg::NotRegularFile => "non è un file normale",
        Msg::NotAFileInFolder => "non è un file nella cartella",
        Msg::PreviewNotUtf8 => "{} non è UTF-8, letto come {}.",
        Msg::Unreadable => "{} non è leggibile: {}",
        Msg::NoHistory => "Ancora nessuna cronologia. «Crea MD5» o «Verifica» crea {}.",
        Msg::NotOurHistory => "{} non è una cronologia di Veritas.",
        Msg::HistoryEmpty => "{} non contiene ancora nessuna esecuzione.",
        Msg::TooLarge => {
            "{} è troppo grande ({n}\u{a0}MiB); Veritas ne legge al massimo {n}\u{a0}MiB. È il file giusto?"
        }
        Msg::FileReadDenied => "Nessun permesso di lettura",
        Msg::FileVanished => "File scomparso durante la lettura",
        Msg::FileReadError => "Errore di lettura: {m}",
        Msg::NotRegular => "Non è un file normale: {}",
        Msg::SymlinkLoop => "Ciclo di collegamenti simbolici: {m}",
        Msg::DirDenied => "Nessun permesso: {m}",
        Msg::DirReadError => "Errore di lettura della cartella: {m}",
        Msg::UnknownPlace => "(sconosciuto)",
        Msg::BadName => "Il nome del file non è UTF-8 valido o si trova fuori dalla cartella",
        Msg::NotChecked => "Non verificato – {m}",
        Msg::RunCancelledRow => "Esecuzione annullata, nessun confronto.",
        Msg::NotADirectory => "Non è una cartella: {}",
        Msg::ReadDeniedAt => "Nessun permesso di lettura: {} ({})",
        Msg::ReadDeniedPlain => "Nessun permesso di lettura: {}",
        Msg::NotFoundAt => "Non trovato: {} ({})",
        Msg::NotFoundPlain => "Non trovato: {}",
        Msg::IoErrorAt => "Errore di input/output: {} ({})",
        Msg::IoErrorPlain => "Errore di input/output: {}",
        Msg::WriteDenied => "Nessun permesso di scrittura: {} ({})",
        Msg::WriteReadOnly => "L’unità è protetta da scrittura: {} ({})",
        Msg::WriteFull => "L’unità è piena: {} ({})",
        Msg::WriteInTheWay => "Un file temporaneo è d’intralcio: {} ({})",
        Msg::WriteFailed => "Errore di scrittura: {} ({})",
        Msg::ChecksumMissing => "Nessun file di checksum trovato: {}. Crearlo con «Crea MD5».",
        Msg::JsonFailed => "Impossibile creare il JSON: {}",
        Msg::RunCancelled => "Esecuzione annullata",
        Msg::ThreadPool => "Impossibile avviare il pool di thread: {}",
        Msg::RenameFailed => {
            "{m}. La cronologia riporta già questa esecuzione; il file di checksum è rimasto invariato."
        }
        Msg::ReuseFromVerify => {
            "Dall’ultima verifica riutilizzati: {p} (dimensione e data di modifica invariate); letti: {p}."
        }
        Msg::ReuseFromGenerate => {
            "Dall’ultima creazione riutilizzati: {p} (dimensione e data di modifica invariate); letti: {p}."
        }
        Msg::KnownStale => {
            "I checksum dell’ultima esecuzione non sono più validi (file di checksum o impostazioni cambiati); sono stati letti tutti i file."
        }
        Msg::NestedKept => {
            "Voci nelle sottocartelle mantenute invariate: {n} («Sottocartelle» è disattivato)."
        }
        Msg::NestedSkipped => {
            "Voci nelle sottocartelle non verificate: {n} («Sottocartelle» è disattivato)."
        }
        Msg::BelowUnreadableKept => {
            "Voci in cartelle non leggibili: {n} – non verificate, mantenute invariate."
        }
        Msg::BelowUnreadable => "Voci in cartelle non leggibili: {n} – non verificate.",
        Msg::AbortedIntegrity => {
            "Annullato durante la verifica dell’integrità. Non è stato scritto nulla."
        }
        Msg::AlgoMissing => "{} manca; questo algoritmo non viene confrontato.",
        Msg::ReplacingInvalid => "Il file di checksum precedente viene sostituito: {m}",
        Msg::ChecksumInvalid => "File di checksum non valido ({}): {m}",
        Msg::NotUtf8Read => "{} non è UTF-8 ed è stato letto come {}.",
        Msg::Line => "riga {n}: {m}",
        Msg::LineWarning => "{}: riga {n}: {m}",
        Msg::HashLenMismatch => "{}: la lunghezza dell’hash non corrisponde a {}: {}",
        Msg::SymlinksSkipped => {
            "Collegamenti simbolici ignorati: {n} (Veritas non segue i collegamenti)."
        }
        Msg::SpecialSkipped => "File speciali ignorati: {n} (non sono file normali).",
        Msg::NothingWritten => "{m} Non è stato scritto nulla.",
        Msg::NotInHistory => "{m} Questa esecuzione non compare nella cronologia.",
        Msg::HistoryUnreadable => "La cronologia {} non è leggibile ({}) e resta invariata.",
        Msg::HistoryOtherFormat => {
            "La cronologia {} ha il formato «{}» di un’altra versione di Veritas; questa versione non la sovrascrive."
        }
        Msg::HistoryReplaced => "{} non era una cronologia di Veritas ed è stato sostituito.",
        Msg::DuplicatePath => "percorso {} presente più volte, vale l’ultimo hash",
        Msg::HashLength => "l’hash deve avere 32 (MD5) o 64 (SHA-256) cifre esadecimali",
        Msg::HashLengthAny => {
            "l’hash deve avere 32 (MD5), 40 (SHA-1), 64 (SHA-256) o 128 (SHA-512) cifre esadecimali"
        }
        Msg::TagLength => "una riga in formato BSD richiede «(file) = hash» con un hash {}",
        Msg::NoSeparator => "dopo l’hash mancano due spazi o un contrassegno binario",
        Msg::PathMissing => "percorso mancante",
        Msg::PathOutside => "il percorso non deve uscire dalla cartella",
        Msg::PathNul => "il percorso contiene un carattere nullo",
        Msg::PathRelative => "il percorso deve essere relativo",
        Msg::PathDrive => "il percorso deve essere relativo (unità o due punti)",
        Msg::ReadDenied => "File non leggibile: nessun permesso",
        Msg::ReadFailed => "File non leggibile: {}",
        Msg::Empty => "Il file è vuoto",
        Msg::HeadUnreadableAny => "Intestazione del file non leggibile",
        Msg::UnknownFormat => "Formato di file sconosciuto, nessuna verifica della struttura",
        Msg::TooShort => "{}: file troppo corto",
        Msg::HeadUnreadable => "{}: intestazione del file non leggibile",
        Msg::TailUnreadable => "{}: fine del file non leggibile",
        Msg::JpegNoSoi => "JPEG: SOI (FF D8) mancante",
        Msg::JpegOk => "JPEG: SOI (FF D8) ed EOI (FF D9) presenti",
        Msg::JpegOkTrailing => "JPEG: SOI (FF D8) ed EOI (FF D9) presenti, poi altri {n}\u{a0}byte",
        Msg::JpegNoEoi => "JPEG: EOI (FF D9) mancante alla fine del file",
        Msg::JpegCut => "JPEG: il file termina prima dei dati dell’immagine",
        Msg::PngNoSignature => "PNG: firma mancante",
        Msg::PngNoIend => "PNG: chunk IEND mancante alla fine del file",
        Msg::PngChunkUnreadable => "PNG: chunk non leggibile",
        Msg::PngChunkPastEnd => "PNG: un chunk supera la fine del file, IEND mancante",
        Msg::PngOk => "PNG: firma e chunk IEND presenti",
        Msg::PngOkTrailing => "PNG: firma e chunk IEND presenti, poi {n}\u{a0}byte aggiunti",
        Msg::PngManyChunks => "PNG: moltissimi chunk, fine non verificata",
        Msg::RiffNoSignature => "{}: firma RIFF mancante",
        Msg::RiffTooLarge => "{}: la dimensione RIFF supera la fine del file",
        Msg::RiffOk => "{}: firma RIFF presente",
        Msg::HeifNoFtyp => "HEIF/HEIC: box ftyp mancante",
        Msg::HeifFtypShort => "HEIF/HEIC: box ftyp incompleto",
        Msg::HeifBrand => "HEIF/HEIC: ftyp con brand {}",
        Msg::HeifNoBrand => "HEIF/HEIC: ftyp senza brand heic/heif/mif1/msf1",
        Msg::TiffShort => "{}: file più corto dell’intestazione",
        Msg::TiffNoSignature => "TIFF: firma II* o MM* mancante",
        Msg::TiffIfdOutside => "{}: l’offset IFD è fuori dal file",
        Msg::TiffLittle => "TIFF: intestazione little-endian e offset IFD validi",
        Msg::TiffBig => "TIFF: intestazione big-endian e offset IFD validi",
        Msg::BigTiffBad => "BigTIFF: intestazione non valida (dimensione dell’offset diversa da 8)",
        Msg::BigTiffOk => "BigTIFF: intestazione e offset IFD validi",
        Msg::Mp4AtomCut => "MP4/MOV: l’atom {} supera la fine del file (troncato)",
        Msg::Mp4Trailing => "MP4/MOV: atom completi, poi {n}\u{a0}byte aggiunti",
        Msg::Mp4CutBeforeMoov => {
            "MP4/MOV: ftyp presente, struttura degli atom troncata prima di moov"
        }
        Msg::Mp4AtomTooLarge => "MP4/MOV: la dimensione dell’atom supera la fine del file",
        Msg::Mp4HeaderUnreadable => "MP4/MOV: intestazione dell’atom non leggibile",
        Msg::Mp4ManyAtoms => "MP4/MOV: moov presente, moltissimi atom, fine non verificata",
        Msg::Mp4Ok => "MP4/MOV: ftyp e moov presenti",
        Msg::Mp4QuickTime => "MP4/MOV: moov presente (QuickTime senza ftyp)",
        Msg::Mp4NoFtyp => "MP4/MOV: box ftyp mancante",
        Msg::Mp4NoMoov => "MP4/MOV: box moov non trovato nell’intervallo cercato",
        Msg::MkvNoHeader => "MKV: intestazione EBML 1A 45 DF A3 mancante",
        Msg::MkvPastEnd => "MKV: l’intestazione EBML supera la fine del file",
        Msg::MkvShort => "MKV: intestazione EBML incompleta",
        Msg::MkvOk => "MKV: intestazione EBML presente",
        Msg::MkvLength => "MKV: lunghezza EBML non leggibile",
        Msg::PdfNoHead => "PDF: intestazione %PDF- mancante",
        Msg::PdfOk => "PDF: intestazione %PDF- e %%EOF nell’ultima finestra (2048\u{a0}byte)",
        Msg::PdfNoEof => "PDF: %%EOF mancante nell’ultima finestra (2048\u{a0}byte)",
        Msg::OfficeShort => "File Office troppo corto per uno ZIP",
        Msg::OfficeUnreadable => "File Office non leggibile",
        Msg::OfficeOle => "File Office cifrato o in formato OLE, struttura non verificata",
        Msg::OfficeNoZip => "Il file Office non inizia con un header ZIP locale",
        Msg::ZipNoEocd => "Fine della directory centrale ZIP (EOCD) mancante",
        Msg::ZipEocdUnreadable => "Fine della directory centrale ZIP non leggibile",
        Msg::ZipDirOutside => "La directory centrale ZIP è fuori dal file",
        Msg::ZipDirHuge => "Directory centrale ZIP molto grande, non verificata",
        Msg::OoxmlOk => "Office Open XML: struttura ZIP e [Content_Types].xml presenti",
        Msg::NoContentTypes => "[Content_Types].xml mancante",
        Msg::ZipManyEntries => "ZIP con moltissime voci, non verificato per intero",
        Msg::ZipDirUnreadableWhy => "Directory centrale ZIP non leggibile: {}",
        Msg::ZipDirUnreadable => "Directory centrale ZIP non leggibile",
        Msg::ZipDirBroken => "Directory centrale ZIP danneggiata",
        Msg::ZipNameUnreadable => "Nome file ZIP non leggibile",
        Msg::ZipLocalMissing => "Header ZIP locale di [Content_Types].xml mancante",
        Msg::ZipLocalUnreadable => "Header ZIP locale non leggibile",
        Msg::ZipLocalBad => "Header ZIP locale di [Content_Types].xml non valido",
        Msg::RunCancelledStructure => "Esecuzione annullata, struttura non verificata",
    }
}
