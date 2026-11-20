//! French.
//!
//! Terms: Prüfen = « Vérifier », Erzeugen = « Créer le MD5 », Prüfsumme = « somme de
//! contrôle », Verlauf = « historique », Lauf = « exécution », Papierkorb = « Corbeille »,
//! Unterordner = « sous-dossiers ». Punctuation: non-breaking space (`\u{a0}`) before `:`,
//! `;`, `?`, `!`, `%` and inside « » (a test checks this), narrow non-breaking space
//! (`\u{202f}`) as thousands separator, typographic apostrophe (’). In French, 0 and 1 are
//! singular ("0 fichier"). What each field shows and how much room it has is documented at
//! `Texts` in `mod.rs`; `msg.rs` explains the placeholders in `msg` (`{}`, `{n}`, `{p}`, `{m}`).

use super::{Msg, Texts};
use crate::history::Change;

fn n(value: usize) -> String {
    super::number(value, "\u{202f}")
}

/// A number with a word; in French, 0 and 1 take the singular.
fn plural(count: usize, one: &str, many: &str) -> String {
    if count < 2 {
        format!("{count} {one}")
    } else {
        format!("{} {many}", n(count))
    }
}

/// The form of a word after the number: singular for 0 and 1.
fn agree(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count < 2 {
        one
    } else {
        many
    }
}

fn files(count: usize) -> String {
    plural(count, "fichier", "fichiers")
}

fn checksums(count: usize) -> String {
    plural(count, "somme de contrôle", "sommes de contrôle")
}

fn entries(count: usize) -> String {
    plural(count, "entrée", "entrées")
}

/// "1 modifié", "2 modifiés", "2 nouveaux"; « mis à jour » stays the same. The words agree
/// with « fichier » (masculine), « erreur » is the noun itself.
fn counted(change: Change, count: usize) -> String {
    let (one, many) = match change {
        Change::Differs => ("modifié", "modifiés"),
        Change::Damaged => ("corrompu", "corrompus"),
        Change::Changed => ("mis à jour", "mis à jour"),
        Change::New => ("nouveau", "nouveaux"),
        Change::Added => ("ajouté", "ajoutés"),
        Change::Missing => ("absent", "absents"),
        Change::Dropped => ("retiré", "retirés"),
        Change::Unreadable => ("erreur", "erreurs"),
    };
    plural(count, one, many)
}

pub static TEXTS: Texts = Texts {
    date_time: "%d/%m/%Y %H:%M",
    date: "%d/%m/%Y",
    thousands: "\u{202f}",
    decimal: ',',
    size_units: ["o", "Ko", "Mo", "Go", "To"],
    percent: |value| format!("{value}\u{a0}%"),
    files,
    checksums,
    entries,
    runs: |count| plural(count, "exécution", "exécutions"),

    key_ctrl: "Ctrl",
    key_enter: "Entrée",
    key_delete: "Suppr",

    about_tip: "À propos de Veritas (F1)",
    language_tip: |name| format!("Langue\u{a0}: {name} (Ctrl+L)"),
    search_hint: "Rechercher",
    search_tip: "Filtrer les noms de fichiers (Ctrl+F)",
    threads_tip: "Threads\u{a0}: nombre de fichiers traités en même temps (Ctrl+, puis ↑/↓ pour \
                  régler). Toujours un seul sur un disque dur (HDD), c’est plus rapide ainsi. \
                  S’applique dès la prochaine exécution.",
    threads_hdd: "Ce dossier se trouve sur un disque dur (HDD)\u{a0}: Veritas traite ici un \
                  fichier après l’autre, le nombre ne s’applique donc pas.",
    locked: "Verrouillé pendant une exécution. Appuyez deux fois sur Échap pour annuler.",
    subfolders_on: "Sous-dossiers\u{a0}: activés. La vérification et la création du MD5 \
                    incluent aussi les fichiers des sous-dossiers. Clic\u{a0}: seulement les \
                    fichiers directement dans le dossier.",
    subfolders_off: "Sous-dossiers\u{a0}: désactivés. La vérification et la création du MD5 ne \
                     prennent que les fichiers directement dans le dossier\u{a0}; les entrées \
                     des sous-dossiers restent inchangées. Clic\u{a0}: inclure les \
                     sous-dossiers.",
    theme_light: "Thème\u{a0}: clair",
    theme_dark: "Thème\u{a0}: sombre",
    theme_system: "Thème\u{a0}: comme le système",
    verify: "Vérifier",
    verify_tip: |md5| format!("Vérifie le dossier d’après {md5} (F5)"),
    generate: "Créer le MD5",
    generate_tip: |md5| {
        format!(
            "Écrit {md5} avec les sommes de contrôle de tous les fichiers du dossier \
             (Ctrl+Entrée)"
        )
    },
    md5_here: "Fichier de sommes de contrôle de ce dossier",
    md5_missing: "Pas encore dans le dossier. «\u{a0}Créer le MD5\u{a0}» le crée.",

    loading: "Le dossier est encore en cours de lecture. Réessayez dans un instant.",
    writing: "Écriture en cours – presque terminé.",
    unread: "L’historique n’a pas pu être lu. Choisissez à nouveau le dossier à gauche.",
    run_busy: "Une exécution est déjà en cours. Appuyez deux fois sur Échap pour l’annuler.",
    verify_needs_md5: |md5| {
        format!("Il n’y a pas encore de {md5} dans le dossier. D’abord «\u{a0}Créer le MD5\u{a0}».")
    },

    pick_folder: "Choisissez un dossier dans l’arborescence.",
    reading_folder: |folder| format!("Lecture de «\u{a0}{folder}\u{a0}»…"),
    folder_unreadable_status: "Le dossier n’a pas pu être lu.",
    folder_status: |md5, count, exists| {
        if exists {
            format!("{md5} · {}", files(count))
        } else {
            format!("Pas encore de {md5} · {}", files(count))
        }
    },
    md5_unchanged: |md5| format!("{md5} reste inchangé."),
    status_generating: |md5| format!("Écriture de {md5}…"),
    status_verifying: |md5| format!("Vérification d’après {md5}…"),
    cancelling: "Annulation…",
    row_history: "toutes les exécutions de ce dossier\u{a0}; double-clic ou Ctrl+H pour les \
                  afficher",
    row_md5: "double-clic pour l’ouvrir",
    md5_then_now: |algo, old, new| format!("{algo} enregistré {old}, maintenant {new}"),
    md5_then_now_lines: |algo, old, new| {
        format!("{algo} enregistré {old}\n{algo} actuel     {new}")
    },
    short_damaged: "contenu modifié, structure peut-être corrompue",
    short_intact: "contenu modifié\u{a0}; début et fin intacts – altération interne non exclue",
    short_unchecked: "contenu modifié\u{a0}; structure de ce type de fichier non vérifiée",
    detail_changed_intact: "Le contenu a changé. Le début et la fin sont intacts – cela \
                            n’exclut pas une altération à l’intérieur du fichier.",
    detail_changed_unchecked: "Le contenu a changé. Veritas ne vérifie pas la structure de ce \
                               type de fichier.",
    detail_damaged: "Le contenu a changé, et la structure est peut-être corrompue.",
    restore_help: "Vous ne l’avez pas modifié vous-même\u{a0}? Restaurez-le alors depuis votre \
                   sauvegarde.",
    structure: |reason| format!("Contrôle de la structure\u{a0}: {reason}"),
    detail_deleted: |md5| {
        format!(
            "Figure dans {md5}, mais n’est plus dans le dossier. Supprimé volontairement\u{a0}? \
             Alors «\u{a0}Créer le MD5\u{a0}» supprime l’entrée."
        )
    },
    detail_new: |md5| {
        format!("Se trouve dans le dossier, mais ne figure pas dans {md5}. «\u{a0}Créer le MD5\u{a0}» l’ajoute.")
    },
    detail_open: |md5| {
        format!("Figure dans {md5}\u{a0}; pas encore vérifié. «\u{a0}Vérifier\u{a0}» (F5) le lit.")
    },
    detail_ok: |md5| format!("Correspond à {md5}."),
    detail_error: "Est-il ouvert ou verrouillé dans un autre programme\u{a0}? Libérez-le alors \
                   et vérifiez à nouveau (F5).",

    chip_all: |keys| format!("Afficher tous les fichiers (Échap, {keys})"),
    chip_only: |label, keys| format!("Afficher seulement «\u{a0}{label}\u{a0}» ({keys})"),
    chip_none: |label| format!("Aucun fichier avec le statut «\u{a0}{label}\u{a0}»"),

    cancelled: "Annulé. Rien n’a été écrit.",
    run_died: "L’exécution s’est arrêtée de façon inattendue.",
    run_not_started: |why| {
        format!("L’exécution n’a pas pu démarrer ({why}). Redémarrez Veritas et réessayez.")
    },
    reused_read: |reused, read| {
        format!(
            "{} {}, {} {}.",
            checksums(reused),
            agree(reused, "réutilisée", "réutilisées"),
            files(read),
            agree(read, "lu", "lus")
        )
    },
    unreadable_files: |count| {
        format!(
            "{} {} (statut «\u{a0}erreur\u{a0}»).",
            files(count),
            agree(count, "illisible", "illisibles")
        )
    },
    written: |md5, count| format!("{md5} écrit\u{a0}: {}.", files(count)),
    replaced: |md5, count| format!("{md5} remplacé\u{a0}: {}.", files(count)),
    unchanged_since: "Rien n’a changé par rapport à l’ancien fichier.",
    changed_since: |parts| format!("Par rapport à l’ancien fichier\u{a0}: {parts}."),
    damage_kept: "Le nouveau fichier a aussi repris l’état peut-être corrompu\u{a0}; \
                  l’historique conserve les anciennes sommes de contrôle.",
    verified_one: |md5| format!("Vérification d’après {md5}\u{a0}: le fichier est ok."),
    verified_all: |md5, count| {
        if count < 2 {
            format!("Vérification d’après {md5}\u{a0}: {} ok.", files(count))
        } else {
            format!(
                "Vérification d’après {md5}\u{a0}: tous les {} sont ok.",
                files(count)
            )
        }
    },
    verified_findings: |md5, parts| format!("Vérification d’après {md5}\u{a0}: {parts}."),
    title_writing: "Écriture en cours",
    title_cancelling: "Annulation en cours",

    crash_notice: |log| {
        format!(
            "Veritas a été interrompu la dernière fois par une erreur du programme. \
             Rapport\u{a0}: {log}"
        )
    },
    drop_running: "Impossible de déposer un dossier pendant une exécution. Appuyez deux fois \
                   sur Échap pour annuler.",
    drop_many: |first| {
        format!("Plusieurs éléments déposés – seul le premier compte\u{a0}: «\u{a0}{first}\u{a0}».")
    },
    drop_file: "Fichier déposé – c’est le dossier qui le contient qui compte.",
    folder_running: "Changez de dossier après l’exécution. Appuyez deux fois sur Échap pour \
                     annuler.",
    not_a_folder: |path| {
        format!(
            "«\u{a0}{path}\u{a0}» n’est pas (ou plus) un dossier. Choisissez-en un autre dans \
             l’arborescence à gauche."
        )
    },
    folder_unreadable: "Le dossier n’a pas pu être lu. Choisissez-le à nouveau dans \
                        l’arborescence à gauche.",
    recent_gone: |path| {
        format!("«\u{a0}{path}\u{a0}» n’existe plus\u{a0}; il a été retiré des derniers dossiers.")
    },
    no_findings: "Aucune anomalie dans la liste.",
    open_gone: |path| format!("{path} est absent et ne peut pas être ouvert."),
    open_refused: |path, why| format!("{path} {why}\u{a0}; impossible de l’ouvrir."),
    open_failed: |path, why| {
        format!(
            "{path} ne peut pas être ouvert ({why}). Un programme est-il associé à ce type de \
             fichier\u{a0}? «\u{a0}Afficher dans le dossier\u{a0}» du menu contextuel peut \
             aider."
        )
    },
    open_json_failed: |path, why| {
        format!(
            "{path} ne peut pas être ouvert ({why}). N’importe quel éditeur ou navigateur \
             affiche aussi ce fichier."
        )
    },
    delete_running: "Rien n’est supprimé pendant une exécution. Appuyez deux fois sur Échap \
                     pour annuler.",
    delete_own: "Veritas ne supprime ni le fichier de sommes de contrôle ni l’historique.",
    delete_refused: |path, why| format!("{path} {why}\u{a0}; Veritas ne le supprime pas."),
    delete_gone: |path| format!("{path} n’est plus dans le dossier."),
    kept: |path| format!("{path} est conservé."),
    trash_failed: |path, why| {
        format!(
            "{path} n’a pas pu être mis à la Corbeille ({why}). Est-il encore ouvert dans un \
             programme\u{a0}? Fermez alors le programme et réessayez."
        )
    },
    trashed: |path| format!("{path} est dans la Corbeille."),
    reveal_gone: |path| format!("{path} est absent\u{a0}; il n’y a rien à afficher."),
    reveal_refused: |path, why| format!("{path} {why}\u{a0}; impossible d’afficher son dossier."),
    reveal_failed: |path, why| format!("Impossible d’afficher le dossier de {path} ({why})."),
    md5_copied: |algo, path| format!("{algo} de {path} copié."),
    close_esc: "Fermer (Échap)",
    close: "Fermer",

    replace_title: |md5| format!("Remplacer {md5}\u{a0}?"),
    replace: "Remplacer",
    keep: "Conserver",
    replace_body: |md5, count, history, advise| {
        let advice = if advise {
            " En cas de doute, vérifiez d’abord (F5)."
        } else {
            ""
        };
        format!(
            "{md5} contient {}. «\u{a0}Remplacer\u{a0}» réécrit le fichier avec l’état actuel \
             du dossier. Cet état sera ensuite considéré comme correct – même pour les \
             fichiers modifiés à votre insu.{advice} L’historique dans {history} consigne \
             les sommes de contrôle qui changent\u{a0}; aucune copie de sauvegarde (.md5.bak) \
             n’est créée.",
            checksums(count)
        )
    },
    replace_reuse: |count, verify, at| {
        let from = if verify {
            "la vérification"
        } else {
            "la création"
        };
        format!(
            "Reprend jusqu’à {} provenant de {from} de {at}\u{a0}; seuls sont lus les \
             fichiers apparus depuis, de taille ou de date de modification différente, ou \
             qui n’étaient pas lisibles.",
            checksums(count)
        )
    },
    replace_nested: |count| {
        format!(
            "{} dans des sous-dossiers {} inchangée{}, car «\u{a0}Sous-dossiers\u{a0}» est \
             désactivé.",
            entries(count),
            agree(count, "reste", "restent"),
            agree(count, "", "s")
        )
    },
    close_writing_body: |folder, md5| {
        format!(
            "Veritas écrit en ce moment le résultat pour «\u{a0}{folder}\u{a0}» (l’historique, \
             et {md5} en cas de création). À la fermeture, la fenêtre attend la fin de \
             l’écriture."
        )
    },
    close_title: "Annuler l’exécution et fermer\u{a0}?",
    close_body: |folder, md5| {
        format!(
            "Veritas calcule en ce moment les sommes de contrôle de «\u{a0}{folder}\u{a0}». \
             Fermer annule l’exécution\u{a0}; rien n’est écrit – ni {md5} ni l’historique."
        )
    },
    cancel_close: "Annuler et fermer",
    keep_running: "Laisser continuer",
    trash_title: |name| format!("Mettre {name} à la Corbeille\u{a0}?"),
    to_trash: "Mettre à la Corbeille",
    trash_body: |path| format!("{path} est mis à la Corbeille et peut y être restauré."),
    trash_body_recorded: |md5| {
        format!(
            "La somme de contrôle reste dans {md5}\u{a0}: le fichier apparaît ensuite comme \
             «\u{a0}absent\u{a0}» dans la liste, jusqu’à ce que «\u{a0}Créer le MD5\u{a0}» \
             supprime l’entrée. L’historique le consigne alors aussi."
        )
    },
    enter_tip: |label| format!("{label} (Entrée)"),
    esc_tip: |label| format!("{label} (Échap)"),

    cancelling_lead: "Annulation – rien n’est écrit…",
    armed_lead: "Appuyez encore sur Échap pour annuler – rien n’est écrit",
    cancel: "Annuler",
    cancel_tip: "Annuler (Échap deux fois). Rien n’est écrit.",
    cancel_late: "Il n’est plus possible d’annuler.",
    files_progress: |done, total| format!("{done} sur {total} fichiers"),
    reused_progress: |count| {
        format!(
            "{} {}",
            checksums(count),
            agree(count, "réutilisée", "réutilisées")
        )
    },
    left: |eta| format!("encore {eta}"),

    tree_running: "Changez de dossier après l’exécution (Échap deux fois annule).",

    list_reading: "Lecture…",
    no_files_sub_on: "Aucun fichier directement dans ce dossier. «\u{a0}Créer le MD5\u{a0}» \
                      inclut les sous-dossiers.",
    no_files_sub_off: "Aucun fichier directement dans ce dossier. Pour les sous-dossiers, \
                       activez «\u{a0}Sous-dossiers\u{a0}» dans la barre d’outils.",
    no_files: "Aucun fichier dans ce dossier.",
    no_hits: |query| format!("Aucun résultat pour «\u{a0}{query}\u{a0}»."),
    no_status: "Aucun fichier avec ce statut.",
    col_status: "Statut",
    col_name: "Nom",
    col_size: "Taille",
    col_modified: "Modifié",
    mark_open: "à vérifier",
    mark_ok: "ok",
    mark_wrong: "modifié",
    mark_damaged: "corrompu",
    mark_deleted: "absent",
    mark_new: "nouveau",
    mark_error: "erreur",
    mark_source: "md5",
    mark_report: "historique",
    change_changed: "mis à jour",
    change_added: "ajouté",
    change_dropped: "retiré",
    counted,
    history_tip: "Afficher toutes les entrées de ce fichier (Ctrl+H)",
    menu_history: "Afficher l’historique",
    menu_open: "Ouvrir",
    menu_file_history: "Historique du fichier",
    menu_reveal: "Afficher dans le dossier",
    menu_copy: |algo| format!("Copier le {algo}"),
    menu_trash: "À la Corbeille …",
    why_gone: "Le fichier est absent.",
    why_no_history: "Il n’y a pas encore d’historique pour ce dossier.",
    why_no_md5: |algo| {
        format!(
            "Il n’y a pas encore de {algo} pour ce fichier. «\u{a0}Vérifier\u{a0}» (F5) le \
             calcule."
        )
    },

    empty_title: "Choisir un dossier",
    empty_text: "Choisissez un dossier dans l’arborescence à gauche ou glissez un dossier dans \
                 la fenêtre.",
    recent_title: "Ouverts récemment",
    open_recent: |path, key| format!("Ouvrir {path} ({key})"),
    keys_title: "Touches",
    empty_keys: [
        ("Ctrl+Entrée", "Créer le MD5"),
        ("F5", "Vérifier"),
        ("Ctrl+H", "Historique\u{a0}: ce qui a changé, et quand"),
        ("Suppr", "Mettre à la Corbeille, avec confirmation"),
        (
            "Échap",
            "retour\u{a0}; deux fois\u{a0}: annuler l’exécution",
        ),
    ],

    about_title: "À propos de Veritas",
    about_close: "Échap / F1 pour fermer",
    about_text: "Veritas vérifie des fichiers et des dossiers avec MD5 et indique si un fichier \
                 multimédia modifié est encore intact sur le plan de la structure – en local, \
                 sans compte et sans cloud. Logiciel libre\u{a0}: le code source est ouvert.",
    about_version: "Version",
    about_copyright: "Copyright",
    about_license: "Licence",
    about_source: "Code source",
    report_title: "Signaler un bogue",
    report_text: "Quelque chose ne fonctionne pas\u{a0}? Décrivez-le dans un ticket GitHub – il \
                  faut pour cela un compte GitHub gratuit. La version et le système sont déjà \
                  renseignés\u{a0}; les noms de fichiers et les chemins ne sont pas \
                  nécessaires.",
    report_link: "Signaler un bogue sur GitHub",
    wish_title: "Souhaits",
    wish_text: "Une idée pour rendre la vérification avec Veritas plus simple ou plus sûre\u{a0}? \
                Écrivez-la sous forme de souhait – là aussi avec un compte GitHub.",
    wish_link: "Écrire un souhait sur GitHub",
    third_title: "Composants tiers",
    third_text: "Veritas utilise des bibliothèques et des polices libres d’autres auteurs\u{a0}: \
                 Noto Sans et DejaVu Sans Mono (Linux), les icônes de Lucide et les polices de \
                 secours d’egui (Windows). Leurs textes de licence sont fournis avec le \
                 programme.",
    third_link: "Tous les composants tiers et licences",

    history_title: "Historique",
    tab_runs: "Exécutions",
    tab_file: |name| format!("Fichier\u{a0}: {name}"),
    action_created: "MD5 créé",
    action_replaced: "MD5 remplacé",
    action_verified: "Vérifié",
    short_created: "créé",
    short_replaced: "remplacé",
    short_verified: "vérifié",
    run_cancelled: "annulée",
    no_change: "aucun changement",
    all_ok: "tout est ok",
    first_facts: |count| {
        format!(
            "{} dans le premier fichier de sommes de contrôle.",
            files(count)
        )
    },
    unchanged_count: |count| {
        let word = if matches!(count, "0" | "1") {
            "inchangé"
        } else {
            "inchangés"
        };
        format!("{count} {word}")
    },
    flag_damaged: "Anomalie\u{a0}: corrompu ou illisible",
    flag_differs: "Anomalie\u{a0}: modifié ou absent",
    baseline_note: "Pour le premier fichier de sommes de contrôle, l’historique ne consigne pas \
                    les fichiers un par un. Chaque fichier qui n’apparaît pas ensuite comme \
                    modifié est encore dans cet état.",
    generate_none: "Rien n’a changé par rapport au fichier .md5 précédent.",
    verify_none: "Tous les fichiers correspondent au fichier de sommes de contrôle.",
    before: "avant",
    after: "après",
    entry_tip: "Un clic affiche toutes les entrées de ce fichier.",
    file_none: |since| {
        format!(
            "Aucune entrée depuis {since}\u{a0}: aucune exécution n’a signalé ce fichier \
             comme modifié, nouveau, absent ou illisible."
        )
    },
    file_events: |count, since| {
        format!(
            "{} depuis {since}, la plus récente en haut.",
            entries(count)
        )
    },
    show_run: "Afficher cette exécution",
    open_json: "Ouvrir en JSON",
    open_json_tip: |name| format!("Ouvrir {name} dans le programme que le système associe au JSON"),
    keys_file: "← / → onglet  ·  clic sur une date pour afficher l’exécution  ·  Échap ferme",
    keys_runs_file: "↑ / ↓ exécution  ·  ← / → onglet  ·  Échap ferme",
    keys_runs: "↑ / ↓ exécution  ·  clic sur un fichier pour afficher ses entrées  ·  Échap ferme",
    hint_none: |since| format!("Historique\u{a0}: aucun changement depuis {since}"),
    hint_last: |what, day| format!("Historique\u{a0}: {what} le {day}"),
    hint_last_many: |what, day, count| {
        format!("Historique\u{a0}: {what} le {day} ({})", entries(count))
    },

    sys_not_found: "introuvable",
    sys_denied: "accès refusé",
    sys_message: |text| format!("message du système\u{a0}: {text}"),
    no_trash_unc: "Les chemins réseau n’ont pas de Corbeille\u{a0}; Veritas n’y supprime rien.",
    no_trash_network: "Les lecteurs réseau n’ont pas de Corbeille\u{a0}; Veritas n’y supprime \
                       rien.",
    no_trash_removable: "Les supports amovibles n’ont pas de Corbeille\u{a0}; Veritas n’y \
                         supprime rien.",

    msg,
};

/// Texts from run, preview and structure check (`Msg`, placeholders as in the catalogue).
/// Where a placeholder is a number with a word (`{p}`) or a number (`{n}`), it is placed so
/// that no word form depends on it (no participles in the plural).
fn msg(msg: Msg) -> &'static str {
    match msg {
        Msg::PhaseReady => "Prêt",
        Msg::PhaseReadMd5 => "Lecture du fichier de sommes de contrôle…",
        Msg::PhaseWalk => "Lecture du dossier…",
        Msg::PhaseMatch => "Rapprochement des sommes de contrôle connues…",
        Msg::PhaseHash => "Calcul des sommes de contrôle…",
        Msg::PhaseIntegrity => "Contrôle de l’intégrité…",
        Msg::PhaseWrite => "Écriture des fichiers…",
        Msg::PhaseDone => "Terminé",
        Msg::NoFolder => "Choisissez d’abord un dossier à gauche.",
        Msg::NotInFolder => "n’est pas dans le dossier choisi",
        Msg::IsLink => "est un lien ou se trouve derrière un lien, et Veritas ne suit pas les liens",
        Msg::NotRegularFile => "n’est pas un fichier ordinaire",
        Msg::NotAFileInFolder => "n’est pas un fichier du dossier",
        Msg::PreviewNotUtf8 => "{} n’est pas en UTF-8, lu comme {}.",
        Msg::Unreadable => "{} est illisible\u{a0}: {}",
        Msg::NoHistory => {
            "Pas encore d’historique. «\u{a0}Créer le MD5\u{a0}» ou «\u{a0}Vérifier\u{a0}» crée {}."
        }
        Msg::NotOurHistory => "{} n’est pas un historique de Veritas.",
        Msg::HistoryEmpty => "{} ne contient encore aucune exécution.",
        Msg::TooLarge => {
            "{} est trop volumineux avec {n}\u{a0}Mio\u{a0}; Veritas lit au plus {n}\u{a0}Mio. Est-ce le bon fichier\u{a0}?"
        }
        Msg::FileReadDenied => "Aucune autorisation de lecture",
        Msg::FileVanished => "Fichier disparu pendant la lecture",
        Msg::FileReadError => "Erreur de lecture\u{a0}: {m}",
        Msg::NotRegular => "Pas un fichier ordinaire\u{a0}: {}",
        Msg::SymlinkLoop => "Boucle de liens symboliques\u{a0}: {m}",
        Msg::DirDenied => "Aucune autorisation\u{a0}: {m}",
        Msg::DirReadError => "Erreur de lecture du dossier\u{a0}: {m}",
        Msg::UnknownPlace => "(inconnu)",
        Msg::BadName => "Le nom de fichier n’est pas de l’UTF-8 valide ou se trouve hors du dossier",
        Msg::NotChecked => "Non vérifié – {m}",
        Msg::RunCancelledRow => "Exécution annulée, pas de comparaison.",
        Msg::NotADirectory => "Pas un dossier\u{a0}: {}",
        Msg::ReadDeniedAt => "Aucune autorisation de lecture\u{a0}: {} ({})",
        Msg::ReadDeniedPlain => "Aucune autorisation de lecture\u{a0}: {}",
        Msg::NotFoundAt => "Introuvable\u{a0}: {} ({})",
        Msg::NotFoundPlain => "Introuvable\u{a0}: {}",
        Msg::IoErrorAt => "Erreur d’entrée/sortie\u{a0}: {} ({})",
        Msg::IoErrorPlain => "Erreur d’entrée/sortie\u{a0}: {}",
        Msg::WriteDenied => "Aucune autorisation d’écriture\u{a0}: {} ({})",
        Msg::WriteReadOnly => "Le support est protégé en écriture\u{a0}: {} ({})",
        Msg::WriteFull => "Le support est plein\u{a0}: {} ({})",
        Msg::WriteInTheWay => "Un fichier temporaire fait obstacle\u{a0}: {} ({})",
        Msg::WriteFailed => "Erreur d’écriture\u{a0}: {} ({})",
        Msg::ChecksumMissing => {
            "Aucun fichier de sommes de contrôle trouvé\u{a0}: {}. Créez-le avec «\u{a0}Créer le MD5\u{a0}»."
        }
        Msg::JsonFailed => "Le JSON n’a pas pu être créé\u{a0}: {}",
        Msg::RunCancelled => "Exécution annulée",
        Msg::ThreadPool => "Le pool de threads n’a pas pu être démarré\u{a0}: {}",
        Msg::RenameFailed => {
            "{m}. L’historique mentionne déjà cette exécution\u{a0}; le fichier de sommes de contrôle est resté inchangé."
        }
        Msg::ReuseFromVerify => {
            "Repris de la dernière vérification (taille et date de modification inchangées)\u{a0}: {p}\u{a0}; lus\u{a0}: {p}."
        }
        Msg::ReuseFromGenerate => {
            "Repris de la dernière création (taille et date de modification inchangées)\u{a0}: {p}\u{a0}; lus\u{a0}: {p}."
        }
        Msg::KnownStale => {
            "Les sommes de contrôle de la dernière exécution ne correspondent plus (fichier de sommes de contrôle ou réglages modifiés)\u{a0}; tous les fichiers ont été lus."
        }
        Msg::NestedKept => {
            "Entrées de sous-dossiers conservées telles quelles\u{a0}: {n} («\u{a0}Sous-dossiers\u{a0}» est désactivé)."
        }
        Msg::NestedSkipped => {
            "Entrées de sous-dossiers non vérifiées\u{a0}: {n} («\u{a0}Sous-dossiers\u{a0}» est désactivé)."
        }
        Msg::BelowUnreadableKept => {
            "Entrées dans des dossiers illisibles, non vérifiées et conservées telles quelles\u{a0}: {n}."
        }
        Msg::BelowUnreadable => "Entrées dans des dossiers illisibles, non vérifiées\u{a0}: {n}.",
        Msg::AbortedIntegrity => "Annulé pendant le contrôle d’intégrité. Rien n’a été écrit.",
        Msg::AlgoMissing => "{} est manquant\u{a0}; cet algorithme n’est pas comparé.",
        Msg::ReplacingInvalid => "Le fichier de sommes de contrôle précédent est remplacé\u{a0}: {m}",
        Msg::ChecksumInvalid => "Fichier de sommes de contrôle invalide ({})\u{a0}: {m}",
        Msg::NotUtf8Read => "{} n’est pas en UTF-8 et a été lu comme {}.",
        Msg::Line => "ligne {n}\u{a0}: {m}",
        Msg::LineWarning => "{}\u{a0}: ligne {n}\u{a0}: {m}",
        Msg::HashLenMismatch => "{}\u{a0}: la longueur du hash ne correspond pas à {}\u{a0}: {}",
        Msg::SymlinksSkipped => {
            "Liens symboliques ignorés\u{a0}: {n} (Veritas ne suit pas les liens)."
        }
        Msg::SpecialSkipped => "Fichiers spéciaux ignorés\u{a0}: {n} (pas des fichiers ordinaires).",
        Msg::NothingWritten => "{m} Rien n’a été écrit.",
        Msg::NotInHistory => "{m} Cette exécution ne figure pas dans l’historique.",
        Msg::HistoryUnreadable => "L’historique {} est illisible ({}) et reste inchangé.",
        Msg::HistoryOtherFormat => {
            "L’historique {} a le format «\u{a0}{}\u{a0}» d’une autre version de Veritas\u{a0}; cette version ne l’écrase pas."
        }
        Msg::HistoryReplaced => "{} n’était pas un historique de Veritas et a été remplacé.",
        Msg::DuplicatePath => "chemin {} en double, le dernier hash compte",
        Msg::HashLength => "le hash doit comporter 32 (MD5) ou 64 (SHA-256) chiffres hexadécimaux",
        Msg::HashLengthAny => {
            "le hash doit comporter 32 (MD5), 40 (SHA-1), 64 (SHA-256) ou 128 (SHA-512) chiffres hexadécimaux"
        }
        Msg::TagLength => {
            "une ligne au format BSD doit avoir «\u{a0}(fichier) = hash\u{a0}» avec un hash {}"
        }
        Msg::NoSeparator => "deux espaces ou un marqueur binaire manquent après le hash",
        Msg::PathMissing => "chemin manquant",
        Msg::PathOutside => "le chemin ne doit pas sortir du dossier",
        Msg::PathNul => "le chemin contient un caractère nul",
        Msg::PathRelative => "le chemin doit être relatif",
        Msg::PathDrive => "le chemin doit être relatif (lecteur ou deux-points)",
        Msg::ReadDenied => "Fichier illisible\u{a0}: aucune autorisation",
        Msg::ReadFailed => "Fichier illisible\u{a0}: {}",
        Msg::Empty => "Fichier vide",
        Msg::HeadUnreadableAny => "En-tête du fichier illisible",
        Msg::UnknownFormat => "Format de fichier inconnu, pas de contrôle de structure",
        Msg::TooShort => "{}\u{a0}: fichier trop court",
        Msg::HeadUnreadable => "{}\u{a0}: en-tête du fichier illisible",
        Msg::TailUnreadable => "{}\u{a0}: fin du fichier illisible",
        Msg::JpegNoSoi => "JPEG\u{a0}: SOI (FF D8) manquant",
        Msg::JpegOk => "JPEG\u{a0}: SOI (FF D8) et EOI (FF D9) présents",
        Msg::JpegOkTrailing => {
            "JPEG\u{a0}: SOI (FF D8) et EOI (FF D9) présents, puis {n}\u{a0}octets de plus"
        }
        Msg::JpegNoEoi => "JPEG\u{a0}: EOI (FF D9) manquant à la fin du fichier",
        Msg::JpegCut => "JPEG\u{a0}: le fichier se termine avant les données d’image",
        Msg::PngNoSignature => "PNG\u{a0}: signature manquante",
        Msg::PngNoIend => "PNG\u{a0}: bloc IEND manquant à la fin du fichier",
        Msg::PngChunkUnreadable => "PNG\u{a0}: bloc illisible",
        Msg::PngChunkPastEnd => "PNG\u{a0}: un bloc dépasse la fin du fichier, IEND manquant",
        Msg::PngOk => "PNG\u{a0}: signature et bloc IEND présents",
        Msg::PngOkTrailing => "PNG\u{a0}: signature et bloc IEND présents, puis {n}\u{a0}octets ajoutés",
        Msg::PngManyChunks => "PNG\u{a0}: très nombreux blocs, fin non vérifiée",
        Msg::RiffNoSignature => "{}\u{a0}: signature RIFF manquante",
        Msg::RiffTooLarge => "{}\u{a0}: la taille RIFF dépasse la fin du fichier",
        Msg::RiffOk => "{}\u{a0}: signature RIFF présente",
        Msg::HeifNoFtyp => "HEIF/HEIC\u{a0}: boîte ftyp manquante",
        Msg::HeifFtypShort => "HEIF/HEIC\u{a0}: boîte ftyp incomplète",
        Msg::HeifBrand => "HEIF/HEIC\u{a0}: ftyp avec la marque {}",
        Msg::HeifNoBrand => "HEIF/HEIC\u{a0}: ftyp sans marque heic/heif/mif1/msf1",
        Msg::TiffShort => "{}\u{a0}: fichier plus court que l’en-tête",
        Msg::TiffNoSignature => "TIFF\u{a0}: signature II* ou MM* manquante",
        Msg::TiffIfdOutside => "{}\u{a0}: le décalage IFD se trouve hors du fichier",
        Msg::TiffLittle => "TIFF\u{a0}: en-tête little-endian et décalage IFD valides",
        Msg::TiffBig => "TIFF\u{a0}: en-tête big-endian et décalage IFD valides",
        Msg::BigTiffBad => "BigTIFF\u{a0}: en-tête invalide (taille de décalage différente de 8)",
        Msg::BigTiffOk => "BigTIFF\u{a0}: en-tête et décalage IFD valides",
        Msg::Mp4AtomCut => "MP4/MOV\u{a0}: l’atome {} dépasse la fin du fichier (tronqué)",
        Msg::Mp4Trailing => "MP4/MOV\u{a0}: atomes complets, puis {n}\u{a0}octets ajoutés",
        Msg::Mp4CutBeforeMoov => {
            "MP4/MOV\u{a0}: ftyp présent, structure d’atomes tronquée avant moov"
        }
        Msg::Mp4AtomTooLarge => "MP4/MOV\u{a0}: la taille d’un atome dépasse la fin du fichier",
        Msg::Mp4HeaderUnreadable => "MP4/MOV\u{a0}: en-tête d’atome illisible",
        Msg::Mp4ManyAtoms => "MP4/MOV\u{a0}: moov présent, très nombreux atomes, fin non vérifiée",
        Msg::Mp4Ok => "MP4/MOV\u{a0}: ftyp et moov présents",
        Msg::Mp4QuickTime => "MP4/MOV\u{a0}: moov présent (QuickTime sans ftyp)",
        Msg::Mp4NoFtyp => "MP4/MOV\u{a0}: boîte ftyp manquante",
        Msg::Mp4NoMoov => "MP4/MOV\u{a0}: boîte moov introuvable dans la zone examinée",
        Msg::MkvNoHeader => "MKV\u{a0}: en-tête EBML 1A 45 DF A3 manquant",
        Msg::MkvPastEnd => "MKV\u{a0}: l’en-tête EBML dépasse la fin du fichier",
        Msg::MkvShort => "MKV\u{a0}: en-tête EBML incomplet",
        Msg::MkvOk => "MKV\u{a0}: en-tête EBML présent",
        Msg::MkvLength => "MKV\u{a0}: longueur EBML illisible",
        Msg::PdfNoHead => "PDF\u{a0}: en-tête %PDF- manquant",
        Msg::PdfOk => "PDF\u{a0}: en-tête %PDF- et %%EOF dans la dernière fenêtre (2048\u{a0}octets)",
        Msg::PdfNoEof => "PDF\u{a0}: %%EOF manquant dans la dernière fenêtre (2048\u{a0}octets)",
        Msg::OfficeShort => "Fichier Office trop court pour un ZIP",
        Msg::OfficeUnreadable => "Fichier Office illisible",
        Msg::OfficeOle => "Fichier Office chiffré ou au format OLE, structure non vérifiée",
        Msg::OfficeNoZip => "Le fichier Office ne commence pas par un en-tête local ZIP",
        Msg::ZipNoEocd => "Fin du répertoire central ZIP (EOCD) manquante",
        Msg::ZipEocdUnreadable => "Fin du répertoire central ZIP illisible",
        Msg::ZipDirOutside => "Le répertoire central ZIP se trouve hors du fichier",
        Msg::ZipDirHuge => "Répertoire central ZIP très volumineux, non vérifié",
        Msg::OoxmlOk => "Office Open XML\u{a0}: structure ZIP et [Content_Types].xml présents",
        Msg::NoContentTypes => "[Content_Types].xml manquant",
        Msg::ZipManyEntries => "ZIP avec très nombreuses entrées, pas entièrement vérifié",
        Msg::ZipDirUnreadableWhy => "Répertoire central ZIP illisible\u{a0}: {}",
        Msg::ZipDirUnreadable => "Répertoire central ZIP illisible",
        Msg::ZipDirBroken => "Répertoire central ZIP corrompu",
        Msg::ZipNameUnreadable => "Nom de fichier ZIP illisible",
        Msg::ZipLocalMissing => "En-tête local ZIP de [Content_Types].xml manquant",
        Msg::ZipLocalUnreadable => "En-tête local ZIP illisible",
        Msg::ZipLocalBad => "En-tête local ZIP de [Content_Types].xml invalide",
        Msg::RunCancelledStructure => "Exécution annulée, structure non vérifiée",
    }
}
