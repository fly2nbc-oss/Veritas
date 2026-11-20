#!/usr/bin/env bash
# quality.sh  erkennt den Stack und prüft ihn (siehe QUALITY.md)
#
# Aufruf:
#   quality.sh lint                        Schutzprüfung, Format, Lint, Typen
#   quality.sh all                         zusätzlich Tests, Testabdeckung und Lücken in
#                                          Abhängigkeiten (osv-scanner; pre-commit, CI)
#   quality.sh coverage-update             wie all; schreibt die gemessene Abdeckung als neue Basis
#   quality.sh approve <ID> <Begründung>   Befund der Schutzprüfung freigeben (nur der Mensch!)
#   quality.sh doctor                      Setup prüfen: Werkzeuge, Versionen, Pfade, Einrichtung
#   quality.sh --version
#
# Master: die Vorlage in einem gemeinsamen Qualitäts-Baukasten. Jedes Projekt hat eine Kopie in
# scripts/quality.sh; die nutzen pre-commit und CI. Die Kopie ist zugleich der
# Schalter: Nur Repos mit scripts/quality.sh werden von Hook und pre-commit geprüft.
#
# Optionale Datei .quality im Projektstamm (Bash, wird eingelesen):
#   PACKAGES=(. photo_lab)                  # Pakete relativ zum Stamm, Standard: (.)
#   FLUTTER_TEST_ARGS="--exclude-tags x"    # zusätzliche Argumente für flutter/dart test
#   COVERAGE_LOCAL=0                        # Abdeckung nur in der CI messen (lange Builds)
#   SKIP="prettier tsc"                     # Prüfungen auslassen, erlaubt sind: fmt clippy test
#                                           # eslint prettier tsc ruff pytest analyze coverage vulns
#                                           # tidy cppcheck (C/C++)
#                                           # (die Schutzprüfung lässt sich bewusst nicht abschalten)
# Weitere Projektdateien:
#   .quality-coverage    Basis der Testabdeckung (steigt automatisch mit, sinkt nur mit Freigabe)
#   .quality-approvals   freigegebene Befunde der Schutzprüfung
#   osv-scanner.toml     freigegebene Lücken in Abhängigkeiten (IgnoredVulns mit reason und ignoreUntil)
# Umgebung:
#   QUALITY_VERBOSE=1    Ausgabe aller Schritte live statt nur bei Fehlern
#   QUALITY_BASE=<ref>   Vergleichsbasis der Schutzprüfung erzwingen
QUALITY_VERSION=1.3.2
set -u

usage() {
  echo "Aufruf: quality.sh [lint|all|coverage-update|doctor|approve <ID> <Begründung>|--version]" >&2
  exit 64
}
mode="${1:-lint}"
case "$mode" in
  lint | all | coverage-update | doctor | approve) ;;
  --version) echo "quality.sh $QUALITY_VERSION"; exit 0 ;;
  *) usage ;;
esac

root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$root" || exit 1

# ---------- Freigabe (approve) ----------
if [ "$mode" = approve ]; then
  shift
  id="${1:-}"
  shift 2>/dev/null
  why="$*"
  [[ "$id" =~ ^[0-9a-f]{10}$ ]] && [ -n "$why" ] || usage
  f="$root/.quality-approvals"
  if [ ! -f "$f" ]; then
    {
      echo "# Freigegebene Befunde der Schutzprüfung (QUALITY.md, Schutzprüfung)."
      echo "# Einträge nur durch den Menschen oder auf seine ausdrückliche Anweisung."
      echo "# Format: <ID>  <Datum>  <Begründung>"
    } >"$f"
  fi
  if grep -q "^$id " "$f"; then
    echo "$id ist bereits freigegeben"
  else
    printf '%s  %s  %s\n' "$id" "$(date +%Y-%m-%d)" "$why" >>"$f"
    echo "freigegeben: $id ($why). .quality-approvals mitcommitten."
  fi
  exit 0
fi

PACKAGES=(.)
FLUTTER_TEST_ARGS=""
COVERAGE_LOCAL=1
SKIP=""
# shellcheck source=/dev/null
[ -f .quality ] && . ./.quality

verbose=0
[ "${QUALITY_VERBOSE:-0}" = 1 ] && verbose=1
cov_file="$root/.quality-coverage"
logbase="${QUALITY_LOGDIR:-${TMPDIR:-/tmp}}"
mkdir -p "$logbase" 2>/dev/null
logdir="$(mktemp -d "$logbase/quality.XXXXXX")"
step=0
pkg=.
found=0
failed=()
summary=()

skip() { [[ " $SKIP " == *" $1 "* ]]; }
fail() {
  echo "!! [$pkg] $1"
  failed+=("[$pkg] $1")
}
need() {
  command -v "$1" >/dev/null 2>&1 && return 0
  fail "$1 fehlt (siehe QUALITY.md, Abschnitt Einrichtung)"
  return 1
}

# Ausgabe eines Schritts nur bei Fehlern zeigen; lange Ausgaben gekürzt, vollständig im Protokoll
show_log() {
  local n
  n=$(wc -l <"$1")
  if [ "$n" -le 200 ]; then
    sed 's/^/   | /' "$1"
  else
    head -n 120 "$1" | sed 's/^/   | /'
    echo "   | … $((n - 160)) Zeilen ausgelassen, vollständige Ausgabe: $1"
    tail -n 40 "$1" | sed 's/^/   | /'
  fi
}
run() {
  local cmd="$*" t0=$SECONDS rc log
  [ "${#cmd}" -gt 100 ] && cmd="${cmd:0:97}..."
  echo ">> [$pkg] $cmd"
  step=$((step + 1))
  log="$logdir/$step.log"
  if [ "$verbose" -eq 1 ]; then
    "$@" 2>&1 | tee "$log"
    rc=${PIPESTATUS[0]}
  else
    "$@" >"$log" 2>&1
    rc=$?
  fi
  if [ "$rc" -eq 0 ]; then
    echo "   ok ($((SECONDS - t0)) s)"
    return 0
  fi
  echo "   FEHLER (Exit $rc, $((SECONDS - t0)) s)"
  [ "$verbose" -eq 1 ] || show_log "$log"
  failed+=("[$pkg] $cmd")
  return "$rc"
}

# pytest endet mit 5, wenn es keine Tests gibt. Das gilt nicht als Fehler.
pytest_ok() {
  pytest "$@"
  local rc=$?
  [ "$rc" -eq 5 ] && { echo "(keine Tests gefunden)"; return 0; }
  return "$rc"
}

# ---------- Testabdeckung ----------
cov_file="$root/.quality-coverage"
cov_results="$logdir/coverage.txt"
: >"$cov_results"
cov_on() {
  [ "$mode" = lint ] && return 1
  skip coverage && return 1
  [ -f "$cov_file" ] || [ "$mode" = coverage-update ] || return 1
  [ "${CI:-}" = true ] || [ "$COVERAGE_LOCAL" = 1 ] || [ "$mode" = coverage-update ]
}
cov_key() { # Unterordner im Paket, Stack
  local p="$pkg/$1"
  p="${p#./}"
  p="${p%/.}"
  [ "$p" = "" ] && p=.
  echo "$p:$2"
}
cov_new() { mktemp -u "$logdir/lcov-XXXXXX"; }
# Zeilenabdeckung in Prozent aus einer LCOV-Datei, ohne generierten Code
lcov_pct() {
  [ -s "$1" ] || return 0
  LC_ALL=C awk '/^SF:/ { f = substr($0, 4); skip = (f ~ /\.(g|freezed|mocks)\.dart$/) }
       /^DA:/ && !skip { split(substr($0, 4), a, ","); total++; if (a[2] + 0 > 0) hit++ }
       END { if (total > 0) printf "%.1f\n", 100 * hit / total }' "$1"
}
cov_base() { [ -f "$cov_file" ] && awk -v k="$1" '$1 == k { print $2 }' "$cov_file"; }
cov_record() { # Schlüssel, LCOV-Datei
  local key="$1" pct base
  pct="$(lcov_pct "$2")"
  base="$(cov_base "$key")"
  if [ -z "$pct" ]; then
    if [ -n "$base" ]; then
      fail "Abdeckung $key: keine Messwerte, Basis ist $base %"
    else
      echo "   Abdeckung $key: keine Messwerte"
    fi
    return
  fi
  echo "$key $pct" >>"$cov_results"
  if [ "$mode" = coverage-update ] || [ -z "$base" ]; then
    echo "   Abdeckung $key: $pct %${base:+ (bisher $base %)}"
  elif LC_ALL=C awk -v c="$pct" -v b="$base" 'BEGIN { exit !(c + 0.05 < b) }'; then
    fail "Abdeckung $key gesunken: $pct % < Basis $base %. Tests ergänzen. Ist das nicht sinnvoll: Mensch fragen (QUALITY.md)"
  else
    echo "   Abdeckung $key: $pct % (Basis $base %)"
  fi
  summary+=("Abdeckung $key: $pct %${base:+ (Basis $base %)}")
}
# Basis fortschreiben: bei all nur anheben, bei coverage-update auch senken
cov_write() {
  [ -s "$cov_results" ] || return 0
  [ "${CI:-}" = true ] && return 0
  local new="$logdir/coverage.new" old="$cov_file"
  [ -f "$old" ] || old=/dev/null
  {
    echo "# Testabdeckung (Zeilen, Prozent) je Paket und Stack; Basis für quality.sh."
    echo "# Steigt automatisch mit. Senken nur mit coverage-update und Freigabe (QUALITY.md)."
    LC_ALL=C awk -v upd="$([ "$mode" = coverage-update ] && echo 1 || echo 0)" '
      FNR == NR { m[$1] = $2; next }
      /^#/ || NF < 2 { next }
      { v = $2; if ($1 in m) { if (upd || m[$1] + 0 > v + 0) v = m[$1]; done[$1] = 1 }; print $1, v }
      END { for (k in m) if (!(k in done)) print k, m[k] }' "$cov_results" "$old" | sort
  } >"$new"
  if ! cmp -s "$new" "$cov_file"; then
    cp "$new" "$cov_file"
    echo "Abdeckungs-Basis aktualisiert: .quality-coverage (bitte mitcommitten)"
  fi
}

# ---------- Schutzprüfung ----------
# Basis: QUALITY_BASE, sonst PR-Ziel in der CI, sonst Abzweigpunkt von main, auf main selbst der letzte Commit
guard_base() {
  # QUALITY_BASE: von Hand oder in der CI der Stand vor dem Push (github.event.before);
  # ungültig (z. B. 000… beim ersten Push eines Branches) zählt wie nicht gesetzt
  if [ -n "${QUALITY_BASE:-}" ] && git rev-parse -q --verify "$QUALITY_BASE^{commit}"; then
    return
  fi
  if [ -n "${GITHUB_BASE_REF:-}" ] && git rev-parse -q --verify "origin/$GITHUB_BASE_REF^{commit}" >/dev/null; then
    git merge-base HEAD "origin/$GITHUB_BASE_REF"
    return
  fi
  local branch ref
  branch="$(git symbolic-ref --short -q HEAD)"
  if [ "$branch" != main ] && [ "$branch" != master ]; then
    for ref in main origin/main master origin/master; do
      if git rev-parse -q --verify "$ref^{commit}" >/dev/null; then
        git merge-base HEAD "$ref"
        return
      fi
    done
  fi
  git rev-parse -q --verify "HEAD^{commit}"
}

# Liest git diff -U0 und meldet neue Ausnahmen, abgeschaltete und entfernte Tests.
# Ausgabe je Befund: Art <TAB> Ort <TAB> Text <TAB> Quelle der ID
GUARD_AWK='
function ext(f,   n, p) { n = split(f, p, "."); return n > 1 ? p[n] : "" }
function trim(s) { gsub(/\t/, " ", s); gsub(/^ +| +$/, "", s); return s }
function check(t, added,   sup, skp, dec) {
  sup = 0; skp = 0; dec = 0
  if (e == "dart") {
    if (t ~ /\/\/[ \t]*ignore(_for_file)?:/) sup = 1
    if (t ~ /withIgnored|withUntracked/) sup = 1   # Leak-Tracker für einzelne Tests abgeschwächt
    if (t ~ /[ \t,(]skip:[ \t]*(true|\047|")/) skp = 1
    if (t ~ /^[ \t]*(test|testWidgets)\(/) dec = 1
  } else if (e == "rs") {
    if (t ~ /#!?\[(allow|expect)\(/ || t ~ /#!?\[cfg_attr\(.*(allow|expect)\(/) sup = 1
    if (t ~ /#\[ignore/) skp = 1
    if (t ~ /#\[([a-z_]+::)?test(\]|\()/) dec = 1
  } else if (e ~ /^(js|jsx|ts|tsx|mjs|cjs|vue)$/) {
    if (t ~ /eslint-disable|@ts-ignore|@ts-expect-error|@ts-nocheck|prettier-ignore/) sup = 1
    if (t ~ /(^|[^A-Za-z0-9_.])(it|test|describe)\.(skip|only|todo)\(/ || t ~ /(^|[^A-Za-z0-9_.])x(it|test|describe)\(/) skp = 1
    if (t ~ /^[ \t]*(it|test)(\.each\(.*\))?\(/) dec = 1
  } else if (e == "py") {
    if (t ~ /#[ \t]*(noqa|type:[ \t]*ignore|ruff:[ \t]*noqa|pragma:[ \t]*no[ \t]*cover|fmt:[ \t]*(off|skip))/) sup = 1
    if (t ~ /pytest\.mark\.(skip|xfail)|pytest\.skip\(|unittest\.skip/) skp = 1
    if (t ~ /^[ \t]*(async[ \t]+)?def[ \t]+test_/) dec = 1
  } else if (e ~ /^(c|cc|cpp|cxx|h|hh|hpp|hxx|ipp|inl)$/) {
    if (t ~ /NOLINT|cppcheck-suppress|clang-format[ \t]+off|no_sanitize|__(asan|lsan|ubsan)_default_(options|suppressions)/) sup = 1
    if (t ~ /(GCC|clang)[ \t]+diagnostic[ \t]+ignored|warning[ \t]*\([ \t]*(disable|suppress)/) sup = 1
    if (t ~ /TEST[A-Z_]*[ \t]*\([^)]*DISABLED_|GTEST_SKIP|(^|[^A-Za-z0-9_])SKIP\(|doctest::skip/) skp = 1
    if (t ~ /(TEST_CASE|SCENARIO)[A-Z_]*[ \t]*\(.*"[^"]*\[(\.|!hide)/) skp = 1
    if (t ~ /^[ \t]*(TEST|TEST_F|TEST_P|TYPED_TEST|TYPED_TEST_P|TEST_CASE|TEST_CASE_METHOD|SCENARIO)[ \t]*\(/) dec = 1
  } else if (file ~ /(^|\/)CMakeLists\.txt$/ || e == "cmake") {
    if (t ~ /(^|[ \t])DISABLED[ \t]+(TRUE|ON|YES|1)|WILL_FAIL|SKIP_RETURN_CODE|SKIP_REGULAR_EXPRESSION/) skp = 1
    if (t ~ /^[ \t]*(add_test|gtest_discover_tests|gtest_add_tests|catch_discover_tests|doctest_discover_tests)[ \t]*\(/) dec = 1
  } else if (e == "cs" && file !~ /\.(g|g\.i|[Dd]esigner|generated)\.cs$|(^|\/)GlobalSuppressions\.cs$/ &&
             file !~ /(^|\/)Migrations\/([0-9]+_[^\/]*|[^\/]*ModelSnapshot)\.cs$/) {
    # C#; generierter Code, EF-Migrationen und GlobalSuppressions.cs (ganz Prüf-Konfiguration) nicht
    if (t ~ /#[ \t]*(pragma[ \t]+warning[ \t]+disable|nullable[ \t]+disable)|\/\/[ \t]*ReSharper[ \t]+disable/) sup = 1
    if (t ~ /(^|[^A-Za-z0-9_])((Unconditional)?SuppressMessage(Attribute)?[ \t]*\(|ExcludeFromCodeCoverage)/) sup = 1
    if (t ~ /(^|[^A-Za-z0-9_.])(null|default)!([^=]|$)/) sup = 1
    if (t ~ /^[ \t]*\[/ && (t ~ /[[,][ \t]*(Ignore|Explicit)[ \t]*([](,]|$)/ || t ~ /[(,][ \t]*(Skip|Ignore|IgnoreReason)[ \t]*=/)) skp = 1
    if (t ~ /Assert\.(Inconclusive|Ignore|Skip|SkipWhen|SkipUnless)[ \t]*\(|(^|[^A-Za-z0-9_.])Skip\.(If|IfNot)[ \t]*\(/) skp = 1
    if (t ~ /^[ \t]*\[/ && t ~ /[[,][ \t]*(Fact|Theory|Test|TestMethod|DataTestMethod|TestCase|TestCaseSource|SkippableFact|SkippableTheory)[ \t]*([](,]|$)/) dec = 1
  }
  if (added) {
    if (sup) print "Ausnahme\t" file ":" line "\t" trim(t) "\tausnahme|" file "|" trim(t)
    if (skp) print "Test abgeschaltet\t" file ":" line "\t" trim(t) "\ttest-aus|" file "|" trim(t)
    if (dec) adds++
  } else if (dec) {
    rems++; removed = removed " | " file ": " trim(t); rfiles[file] = 1
  }
}
/^\+\+\+ / { file = substr($0, 5); sub(/^b\//, "", file); e = ext(file); next }
/^--- / { next }
/^@@ / { match($0, /\+[0-9]+/); line = substr($0, RSTART + 1, RLENGTH - 1) + 0; next }
/^\+/ { check(substr($0, 2), 1); line++; next }
/^-/ { check(substr($0, 2), 0); next }
END {
  if (rems > adds) {
    fl = ""; for (f in rfiles) fl = fl (fl == "" ? "" : ", ") f
    printf "Tests entfernt\t%s\t%d Testfälle weniger als in der Basis\ttests|%s\n", fl, rems - adds, removed
  }
}'

# Dateien, deren Änderung die Prüfung selbst abschwächen kann
is_guard_config() {
  case "$1" in
    .quality | scripts/quality.sh | *analysis_options.yaml | *dart_test.yaml | *clippy.toml | *rustfmt.toml | \
      *osv-scanner.toml | \
      *eslint.config.* | *.eslintrc* | *.prettierrc* | *prettier.config.* | *.prettierignore | \
      *pytest.ini | *.coveragerc | *ruff.toml | *vitest.config.* | *jest.config.* | \
      *.clang-tidy | *.clang-format | *.clang-format-ignore | *_clang-format | *CMakePresets.json | \
      *flutter_test_config.dart | \
      .github/workflows/*)
      return 0
      ;;
    *.editorconfig | *.globalconfig | *Directory.Build.props | *Directory.Build.targets | *Directory.Build.rsp | \
      *.ruleset | *.runsettings | *GlobalSuppressions.cs) # C# / .NET
      return 0
      ;;
  esac
  return 1
}
# Gemischte Dateien: nur Zeilen mit Prüf-Bezug zählen
is_mixed_config() {
  case "$1" in
    *pyproject.toml | *package.json | *Cargo.toml | *tsconfig*.json | *setup.cfg) return 0 ;;
    *CMakeLists.txt | *.cmake) return 0 ;;
    *.csproj | *.props | *.targets | *.sln | *.slnx) return 0 ;; # C# / .NET
  esac
  return 1
}
GUARD_KEYWORDS='ruff|pytest|lint|clippy|eslint|prettier|coverage|noqa|ignore|exclude|allow|warn|deny|strict|noImplicit|skipLibCheck|"test"[[:space:]]*:'
# CMake: Warnungen (-Wall, -w, /W4, /wd…), Warnungen als Fehler, Sanitizer, clang-tidy/cppcheck
# Strenge Profile (QUALITY.md §9): auch das Entfernen dieser Einträge ist eine Abschwächung
GUARD_KEYWORDS+='|forbid|unsafe_code|noUnchecked|noFallthrough|noImplicit|exactOptional|select|filterwarnings|^[+-][[:space:]]*workspace[[:space:]]*='
GUARD_KEYWORDS+='|Werror|WARNING_AS_ERROR|-W[a-z][a-z-]|(^|[[:space:]"(])-w([[:space:]")]|$)|/W[0-4X]|/wd[0-9]|fsanitize|fno-sanitize|SANITIZ|SAN_OPTIONS|CLANG_TIDY|CPPCHECK'

guard() {
  git rev-parse --is-inside-work-tree >/dev/null 2>&1 || return
  local base
  base="$(guard_base 2>/dev/null)"
  if [ -z "$base" ]; then
    echo ">> [guard] Schutzprüfung übersprungen (noch kein Commit)"
    return
  fi
  if ! git cat-file -e "$base:scripts/quality.sh" 2>/dev/null; then
    echo ">> [guard] Schutzprüfung übersprungen: Einführung (Basis hat noch kein scripts/quality.sh)"
    return
  fi
  echo ">> [guard] Schutzprüfung gegen $(git rev-parse --short "$base")"
  local findings="$logdir/guard.tsv" f lines
  : >"$findings"

  git diff --no-color --no-ext-diff -U0 "$base" -- . \
    ':!:*.g.dart' ':!:*.freezed.dart' ':!:*.mocks.dart' ':!:*.min.js' ':!:vendor/**' ':!:third_party/**' ':!:external/**' |
    awk "$GUARD_AWK" >>"$findings"
  guard_secrets "$base" >>"$findings"

  while IFS= read -r f; do
    if is_guard_config "$f"; then
      lines="$(git diff --no-color -U0 "$base" -- "$f" | grep -E '^[+-]' | grep -vE '^(\+\+\+|---) ')"
      [ -z "$lines" ] && lines="(Datei neu oder gelöscht)"
      printf 'Prüf-Konfiguration\t%s\t%s\tconfig|%s|%s\n' "$f" \
        "$(printf '%s' "$lines" | head -n 3 | tr '\t\n' '  ')" "$f" "$(printf '%s' "$lines" | tr '\t\n' '  ')" >>"$findings"
    elif is_mixed_config "$f"; then
      lines="$(git diff --no-color -U0 "$base" -- "$f" | grep -E '^[+-]' | grep -vE '^(\+\+\+|---) ' |
        grep -E "$GUARD_KEYWORDS")"
      if [ -n "$lines" ]; then
        printf 'Prüf-Konfiguration\t%s\t%s\tconfig|%s|%s\n' "$f" \
          "$(printf '%s' "$lines" | head -n 3 | tr '\t\n' '  ')" "$f" "$(printf '%s' "$lines" | tr '\t\n' '  ')" >>"$findings"
      fi
    fi
  done < <(git diff --name-only "$base" -- .)

  # Abdeckungs-Basis gesenkt oder Eintrag entfernt
  if git cat-file -e "$base:.quality-coverage" 2>/dev/null; then
    git show "$base:.quality-coverage" | LC_ALL=C awk -v cur="$cov_file" '
      BEGIN { while ((getline l < cur) > 0) { split(l, a, " "); if (l !~ /^#/ && a[2] != "") now[a[1]] = a[2] } }
      /^#/ || NF < 2 { next }
      !($1 in now) { printf "Abdeckung gesenkt\t%s\t%s %% -> entfernt\tcoverage|%s|%s|-\n", $1, $2, $1, $2; next }
      now[$1] + 0.05 < $2 + 0 { printf "Abdeckung gesenkt\t%s\t%s %% -> %s %%\tcoverage|%s|%s|%s\n", $1, $2, now[$1], $1, $2, now[$1] }' >>"$findings"
  fi

  local kind where text src id open=0 approved=0 report="$logdir/guard.out"
  : >"$report"
  while IFS=$'\t' read -r kind where text src; do
    [ -n "$kind" ] || continue
    id="$(printf '%s' "$src" | git hash-object --stdin | cut -c1-10)"
    if [ -f .quality-approvals ] && grep -q "^$id " .quality-approvals; then
      approved=$((approved + 1))
      summary+=("Freigegeben: $kind $where ($id)")
    else
      open=$((open + 1))
      # Geheimnisse nie im Klartext ausgeben, auch nicht in anderen Befunden
      [ "$kind" = Geheimnis ] || ! secret_match "$text" || text="${text%%"$sec_val"*}$sec_pre…"
      printf '   | %s  %-18s %s  %s\n' "$id" "$kind" "$where" "${text:0:120}" >>"$report"
    fi
  done <"$findings"

  if git diff --name-only "$base" -- .quality-approvals | grep -q .; then
    echo "   Hinweis: .quality-approvals wurde geändert. Neue Freigaben (bitte prüfen):"
    git diff --no-color -U0 "$base" -- .quality-approvals | grep -E '^\+[0-9a-f]' | sed 's/^+/   | /'
    summary+=("Freigaben geändert: .quality-approvals")
  fi
  [ "$approved" -gt 0 ] && echo "   $approved freigegebene Befunde"
  if [ "$open" -eq 0 ]; then
    echo "   ok"
    return
  fi
  echo "   FEHLER: $open Befunde ohne Freigabe"
  cat "$report"
  grep -q '  Geheimnis ' "$report" &&
    echo "   Geheimnis: aus dem Code nehmen (Umgebungsvariable, nicht versionierte Datei). War er schon committet oder gepusht: beim Anbieter erneuern."
  echo "   Ursache beheben statt freigeben. Ist die Ausnahme wirklich nötig: den Menschen fragen."
  echo "   Freigabe nur durch den Menschen: scripts/quality.sh approve <ID> \"<Begründung>\""
  failed+=("[guard] $open Befunde ohne Freigabe")
}

# ---------- Geheimnisse (Teil der Schutzprüfung) ----------
# Regeln "Art|ERE": Gruppe 2 ist der Fund, Gruppe 3 sein fester, nicht geheimer Anfang.
# Nur dieser Anfang erscheint in der Ausgabe. Muster mit Wiederholungen prüft Bash, nicht awk (mawk).
SECRET_RULES=(
  'privater Schlüssel|(^|[^A-Za-z0-9])((-----BEGIN)( [A-Z0-9]+)* PRIVATE KEY( BLOCK)?-----)'
  'AWS-Schlüssel|(^|[^A-Za-z0-9])((AKIA)[0-9A-Z]{16})([^A-Za-z0-9]|$)'
  'GitHub-Token|(^|[^A-Za-z0-9_])((gh[pousr]_)[A-Za-z0-9]{36})([^A-Za-z0-9_]|$)'
  'GitHub-Token|(^|[^A-Za-z0-9_])((github_pat_)[A-Za-z0-9_]{40,})'
  'GitLab-Token|(^|[^A-Za-z0-9_-])((glpat-)[A-Za-z0-9_.-]{20,})'
  'Anthropic-Schlüssel|(^|[^A-Za-z0-9_-])((sk-ant-)[A-Za-z0-9_-]{32,})'
  'OpenAI-Schlüssel|(^|[^A-Za-z0-9_-])((sk-(proj-|svcacct-|admin-)?)[A-Za-z0-9_-]{32,})'
  'Slack-Token|(^|[^A-Za-z0-9_-])((xox[baprs]-)[A-Za-z0-9-]{10,})'
  'Google-API-Schlüssel|(^|[^A-Za-z0-9_-])((AIza)[A-Za-z0-9_-]{35})([^A-Za-z0-9_-]|$)'
  'Stripe-Schlüssel|(^|[^A-Za-z0-9_])(([rs]k_live_)[A-Za-z0-9]{16,})'
)
# Allgemeine Zuweisung wie password = "…": Gruppe 2 Name, Gruppe 5 Wert (mind. 12 Zeichen, in Anführungszeichen)
SECRET_ASSIGN=$'(^|[^A-Za-z0-9_])([A-Za-z0-9_.-]*(password|passwd|passphrase|secret|token|api[_-]?key|access[_-]?key|private[_-]?key))["\']?[[:space:]]*(:=|=>|=|:)[[:space:]]*["\']([^"\'[:space:]]{12,})["\']'
# Keine Befunde (ohne Rücksicht auf Groß- und Kleinschreibung): Platzhalter, Beispielwerte,
# bei Zuweisungen zusätzlich Variablen, Vorlagen und Zeilen, die Umgebungsvariablen lesen
SECRET_PLACEHOLDER='example|sample|dummy|placeholder|changeme|change_me|your[_-]|xxxx|fake|test|redacted|0000000000|\.\.\.|\*\*\*'
SECRET_NOT_VALUE='secret|passw|token|todo|replace|insert|\$\{|\{\{|<|>|%s|://|^\$'
SECRET_ENV='environ|getenv|process\.env|import\.meta\.env|env::var|Platform\.environment|ENV\['
# Vorfilter für secret_match (grep -i), damit Bash nur wenige Zeilen einzeln prüft
SECRET_HINTS='PRIVATE KEY|AKIA|gh[pousr]_|github_pat_|glpat-|sk-|xox[baprs]-|AIza|k_live_|pass|secret|token|api.?key|access.?key|private.?key'
# Neue Zeilen aus git diff -U0: Datei <TAB> Zeile <TAB> Text
ADDED_AWK='/^\+\+\+ / { file = substr($0, 5); sub(/^b\//, "", file); next }
/^@@ / { match($0, /\+[0-9]+/); line = substr($0, RSTART + 1, RLENGTH - 1) + 0; next }
/^\+/ { print file "\t" line "\t" substr($0, 2); line++ }'

imatch() { # Text, ERE: Treffer ohne Rücksicht auf Groß- und Kleinschreibung
  local rc
  shopt -s nocasematch
  [[ $1 =~ $2 ]]
  rc=$?
  shopt -u nocasematch
  return "$rc"
}
# Sucht in $1 ein Geheimnis. Treffer: Rückgabe 0, gesetzt sind sec_kind (Art), sec_val (Fund),
# sec_pre (fester Anfang des Funds) und sec_show (Anzeige ohne das Geheimnis)
secret_match() {
  local r re key LC_ALL=C # Bereiche wie [A-Z] unabhängig von der Spracheinstellung
  sec_kind="" sec_val="" sec_pre="" sec_show=""
  for r in "${SECRET_RULES[@]}"; do
    re="${r#*|}"
    [[ $1 =~ $re ]] || continue
    sec_kind="${r%%|*}" sec_val="${BASH_REMATCH[2]}" sec_pre="${BASH_REMATCH[3]}"
    # echte Schlüssel enthalten Ziffern; Platzhalter wie ghp_xxxx… oder AKIA…EXAMPLE zählen nicht
    if [ "$sec_pre" != "-----BEGIN" ]; then
      [[ $sec_val =~ [0-9] ]] && ! imatch "$sec_val" "$SECRET_PLACEHOLDER" || continue
    fi
    sec_show="$sec_kind $sec_pre…"
    return 0
  done
  imatch "$1" "$SECRET_ASSIGN" || return 1
  key="${BASH_REMATCH[2]}" sec_val="${BASH_REMATCH[5]}"
  # Wert mit Buchstaben und Ziffern, aber kein Name einer Umgebungsvariablen (API_KEY_2)
  [[ $sec_val =~ [A-Za-z] && $sec_val =~ [0-9] && ! $sec_val =~ ^[A-Z][A-Z0-9_]*$ ]] || return 1
  ! imatch "$sec_val" "$SECRET_PLACEHOLDER|$SECRET_NOT_VALUE" && ! imatch "$1" "$SECRET_ENV" || return 1
  sec_kind="Zugangsdaten" sec_pre="" sec_show="Zugangsdaten $key = …"
  return 0
}
# Dateien, die typischerweise Schlüssel oder Passwörter enthalten (Vorlagen wie .env.example nicht)
is_secret_file() {
  case "${1##*/}" in
    *.example | *.sample | *.template | *.dist | *.defaults) return 1 ;;
    .env | .env.* | *.env | *.pem | *.key | *.p12 | *.pfx | *.jks | *.keystore | \
      id_rsa | id_dsa | id_ecdsa | id_ed25519) return 0 ;;
  esac
  return 1
}
# Befunde "Geheimnis" für guard(). Die ID hängt am Hash des Funds, nicht an der Zeilennummer.
# Ausgenommen: Lockfiles, minifizierter und fremder Code, Firebase-Konfiguration (öffentliche Client-Schlüssel)
guard_secrets() { # Basis
  local f ln t
  while IFS= read -r f; do
    is_secret_file "$f" &&
      printf 'Geheimnis\t%s\tneue Datei, enthält typischerweise Schlüssel oder Passwörter\tgeheim-datei|%s\n' "$f" "$f"
  done < <(git diff --name-only --diff-filter=A "$1" -- .)
  git diff --no-color --no-ext-diff -U0 "$1" -- . ':!:*.lock' ':!:*-lock.json' ':!:*-lock.yaml' ':!:*.lock.json' \
    ':!:*.min.js' ':!:vendor/**' ':!:*firebase_options.dart' ':!:*google-services.json' ':!:*GoogleService-Info.plist' |
    awk "$ADDED_AWK" | grep -iE "$SECRET_HINTS" |
    while IFS=$'\t' read -r f ln t; do
      secret_match "$t" || continue
      printf 'Geheimnis\t%s:%s\t%s\tgeheim|%s|%s|%s\n' "$f" "$ln" "$sec_show" "$f" "$sec_kind" \
        "$(printf '%s' "$sec_val" | git hash-object --stdin)"
    done
}

# ---------- Lücken in Abhängigkeiten (osv-scanner) ----------
# Lockfiles, die osv-scanner kennt (Dateinamen wie in den osv-scalibr-Extraktoren; vcpkg kennt er nicht)
LOCKFILE_RE='(^|/)(Cargo\.lock|package-lock\.json|npm-shrinkwrap\.json|yarn\.lock|pnpm-lock\.yaml|bun\.lock|pubspec\.lock|poetry\.lock|uv\.lock|Pipfile\.lock|pdm\.lock|pylock\.toml|[^/]*requirements[^/]*\.txt|packages\.lock\.json|packages\.config|conan\.lock|go\.mod|composer\.lock|Gemfile\.lock|gems\.locked|mix\.lock|gradle\.lockfile|renv\.lock)$'
# Meldungen, die auf fehlendes Netz oder einen Ausfall von osv.dev deuten
OSV_NET_RE='max retries exceeded|dial tcp|no such host|network is unreachable|connection refused|connection reset|i/o timeout|tls handshake|name resolution|deadline exceeded|server error: status|too many requests|proxyconnect'
# Freigaben in osv-scanner.toml brauchen reason und ein Ablaufdatum (ohne Anführungszeichen):
# jede [[IgnoredVulns]] ignoreUntil, jede [[PackageOverrides]], die Lücken ausblendet, effectiveUntil
OSV_TOML_AWK='function check() { if (inb && ign && !(why && until_)) print (id == "" ? "?" : id) }
function start(i, n) { check(); inb = 1; ign = i; id = n; why = 0; until_ = 0 }
/^[ \t]*\[\[[ \t]*IgnoredVulns[ \t]*\]\]/ { start(1, ""); next }
/^[ \t]*\[\[[ \t]*PackageOverrides[ \t]*\]\]/ { start(0, "PackageOverrides"); next }
/^[ \t]*\[/ { check(); inb = 0; next }
inb && /^[ \t]*(id|name)[ \t]*=/ { v = $0; sub(/^[^=]*=[ \t]*/, "", v); sub(/[ \t]*#.*$/, "", v); gsub(/["\047]/, "", v); id = (id == "" ? v : id " " v) }
inb && /^[ \t]*(ignore|vulnerability\.ignore)[ \t]*=[ \t]*true/ { ign = 1 }
inb && /^[ \t]*reason[ \t]*=[ \t]*["\047]+[^"\047 \t]/ { why = 1 }
inb && /^[ \t]*(ignoreUntil|effectiveUntil)[ \t]*=[ \t]*[0-9]/ { until_ = 1 }
END { check() }'
# Wertet die JSON-Ausgabe von osv-scanner v2 aus (pkg/models/results.go). Je Gruppe gleicher Lücken
# eine Zeile: Stufe <TAB> ID <TAB> Schwere <TAB> Paket <TAB> Version <TAB> behoben in <TAB> Lockfile.
# Stufe "hoch": CVSS ab 7 (max_severity); ohne CVSS die Einstufung HIGH oder CRITICAL der Datenbank.
# Alles andere ist ein Hinweis, auch unbekannte Schwere und nicht aufgerufener Code (Aufrufanalyse).
OSV_PY='import json, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
locks = sys.argv[2:]
rank = {"LOW": 1, "MODERATE": 2, "MEDIUM": 2, "HIGH": 3, "CRITICAL": 4}
def rel(p):
    q = p.replace("\\", "/")
    for lf in locks:
        if q == lf or q.endswith("/" + lf):
            return lf
    return p
for res in data.get("results") or []:
    src = rel((res.get("source") or {}).get("path") or "?")
    for pv in res.get("packages") or []:
        pkg = pv.get("package") or {}
        name = pkg.get("name") or "?"
        vulns = {v.get("id"): v for v in pv.get("vulnerabilities") or []}
        for g in pv.get("groups") or []:
            ids = g.get("ids") or []
            if not ids:
                continue
            label, fixed = "", []
            for i in ids:
                v = vulns.get(i) or {}
                s = str((v.get("database_specific") or {}).get("severity") or "").upper()
                if rank.get(s, 0) > rank.get(label, 0):
                    label = s
                for a in v.get("affected") or []:
                    if (a.get("package") or {}).get("name") != name:
                        continue
                    for r in a.get("ranges") or []:
                        if r.get("type") == "GIT":
                            continue
                        for e in r.get("events") or []:
                            if e.get("fixed") and e["fixed"] not in fixed:
                                fixed.append(e["fixed"])
            score = g.get("max_severity") or ""
            try:
                high = float(score) >= 7.0
            except ValueError:
                high = rank.get(label, 0) >= 3
            an = g.get("experimental_analysis") or g.get("experimentalAnalysis") or {}
            quiet = bool(an) and all(not a.get("called", True) or a.get("unimportant") for a in an.values())
            sev = (score or label or "unbekannt") + (", nicht aufgerufen" if quiet else "")
            row = ["hoch" if high and not quiet else "hinweis", ids[0], sev, name,
                   pkg.get("version") or "?", "behoben in " + ", ".join(fixed[:3]) if fixed else "kein Fix bekannt", src]
            print("\t".join(row))'

# Gehört der Ordner zu einem Paket aus PACKAGES (oder zu dessen src-tauri)?
in_packages() {
  local p
  for p in "${PACKAGES[@]}"; do
    p="${p#./}"
    p="${p%/}"
    [ -n "$p" ] || p=.
    [ "$1" = "$p" ] && return 0
    [ "$p" = . ] && p="" || p="$p/"
    [ "$1" = "${p}src-tauri" ] && return 0
  done
  return 1
}
# Lockfiles im Arbeitsbaum (versioniert oder neu, ohne .gitignore-Treffer), relativ zum Stamm.
# Lockfiles verschachtelter Pakete ohne PACKAGES-Eintrag (eigene pubspec.yaml, package.json,
# pyproject.toml oder Cargo.toml, z. B. spikes/) bleiben außen vor, wie bei den übrigen Prüfungen.
vuln_lockfiles() {
  local f d
  git ls-files -co --exclude-standard 2>/dev/null | grep -E "$LOCKFILE_RE" | sort -u | while IFS= read -r f; do
    [ -f "$f" ] || continue
    d="$(dirname "$f")"
    while ! in_packages "$d" && [ "$d" != . ]; do
      if [ -f "$d/pubspec.yaml" ] || [ -f "$d/package.json" ] || [ -f "$d/pyproject.toml" ] || [ -f "$d/Cargo.toml" ]; then
        continue 2
      fi
      d="$(dirname "$d")"
    done
    echo "$f"
  done
}
find_python() {
  local c
  for c in python3 python py; do
    command -v "$c" >/dev/null 2>&1 && "$c" -c 'import json' </dev/null >/dev/null 2>&1 && { echo "$c"; return 0; }
  done
  return 1
}
osv_version() { osv-scanner --version 2>&1 | sed -n 's/^osv-scanner version: v*\([0-9][0-9.]*\).*/\1/p' | head -n 1; }
# Lokal nur ein Hinweis (die CI prüft), in der CI ein Fehler
vuln_unchecked() {
  if [ "${CI:-}" = true ]; then
    fail "$1 (in der CI Pflicht, siehe QUALITY.md, Sicherheit)"
  else
    echo "   Hinweis: $1. Lücken in Abhängigkeiten bleiben lokal ungeprüft, die CI prüft sie."
  fi
}
vuln_rows() { # liest Zeilen aus OSV_PY
  local lvl id sev name ver fix src
  while IFS=$'\t' read -r lvl id sev name ver fix src; do
    printf '   | %-22s %-18s %s %s (%s)  %s\n' "$id" "$sev" "$name" "$ver" "$fix" "$src"
  done
}

# Lücken in Abhängigkeiten mit osv-scanner, einmal pro Repo (all, coverage-update).
# Rot nur bei hohem Risiko; Freigabe einzelner Lücken in osv-scanner.toml (QUALITY.md, Sicherheit).
vulns() {
  local pkg=vulns lf=() cfg=() args=() f py bad rc v t0=$SECONDS
  local json="$logdir/osv.json" err="$logdir/osv.log" rows="$logdir/osv.tsv" high low ign
  skip vulns && return 0
  mapfile -t lf < <(vuln_lockfiles)
  [ "${#lf[@]}" -gt 0 ] || return 0
  echo ">> [vulns] Lücken in Abhängigkeiten (osv-scanner, ${#lf[@]} Lockfiles)"
  while IFS= read -r f; do
    bad="$(awk "$OSV_TOML_AWK" "$f" | tr '\n' ' ')"
    [ -z "$bad" ] || fail "$f: Freigabe ohne reason oder Ablaufdatum (ignoreUntil, effectiveUntil): $bad"
  done < <(git ls-files -co --exclude-standard 2>/dev/null | grep -E '(^|/)osv-scanner\.toml$')
  if ! command -v osv-scanner >/dev/null 2>&1; then
    vuln_unchecked "osv-scanner fehlt (Manjaro: sudo pacman -S osv-scanner; Windows: winget install Google.OSVScanner)"
    return
  fi
  v="$(osv_version)"
  if [ -n "$v" ] && [ "${v%%.*}" -lt 2 ]; then
    vuln_unchecked "osv-scanner $v ist zu alt, nötig ist Version 2"
    return
  fi
  py="$(find_python)" || { vuln_unchecked "Python fehlt zum Auswerten von osv-scanner"; return; }
  # Eine Datei im Stamm gilt für alle Lockfiles (sonst nur für die im selben Ordner)
  [ -f osv-scanner.toml ] && cfg=(--config osv-scanner.toml)
  for f in "${lf[@]}"; do args+=(-L "$f"); done
  osv-scanner scan source --format json "${cfg[@]}" "${args[@]}" >"$json" 2>"$err"
  rc=$?
  [ "$verbose" -eq 1 ] && sed 's/^/   | /' "$err"
  case "$rc" in
    0 | 1) ;;
    128)
      echo "   ok, keine Pakete in den Lockfiles ($((SECONDS - t0)) s)"
      return
      ;;
    *)
      if grep -qiE "$OSV_NET_RE" "$err"; then
        vuln_unchecked "keine Verbindung zu osv.dev"
        [ "${CI:-}" = true ] && show_log "$err"
      else
        echo "   FEHLER (osv-scanner Exit $rc, $((SECONDS - t0)) s)"
        show_log "$err"
        failed+=("[vulns] osv-scanner Exit $rc")
      fi
      return
      ;;
  esac
  if ! PYTHONIOENCODING=utf-8 "$py" -I -c "$OSV_PY" "$json" "${lf[@]}" >"$rows.raw" 2>>"$err"; then
    echo "   FEHLER: Ausgabe von osv-scanner nicht lesbar"
    show_log "$err"
    failed+=("[vulns] Ausgabe von osv-scanner nicht lesbar")
    return
  fi
  tr -d '\r' <"$rows.raw" >"$rows" # Python unter Windows endet Zeilen mit \r\n
  high="$(grep -c '^hoch' "$rows")"
  low="$(grep -c '^hinweis' "$rows")"
  ign="$(sed -n 's/.*Filtered \([0-9][0-9]*\) vulnerabilit.*/\1/p' "$err" | head -n 1)"
  if [ -n "$ign" ]; then
    echo "   $ign Meldungen per osv-scanner.toml ausgeblendet"
    summary+=("Meldungen per osv-scanner.toml ausgeblendet: $ign")
  fi
  if [ "$low" -gt 0 ]; then
    echo "   Hinweis: $low Lücken mit niedrigem oder unbekanntem Risiko (nicht rot), bei Gelegenheit aktualisieren:"
    grep '^hinweis' "$rows" | head -n 20 | vuln_rows
    [ "$low" -gt 20 ] && echo "   | … und $((low - 20)) weitere"
    summary+=("Lücken mit niedrigem oder unbekanntem Risiko: $low")
  fi
  if [ "$high" -eq 0 ]; then
    echo "   ok ($((SECONDS - t0)) s)"
    return
  fi
  echo "   FEHLER: $high Lücken mit hohem Risiko (CVSS ab 7 oder HIGH/CRITICAL)"
  grep '^hoch' "$rows" | vuln_rows
  echo "   Abhängigkeit aktualisieren (cargo update -p, npm update, flutter pub upgrade, uv lock -P …), Lockfile mitcommitten."
  echo "   Geht das nicht: den Menschen fragen. Freigabe nur durch ihn, in osv-scanner.toml (QUALITY.md, Sicherheit)."
  failed+=("[vulns] $high Lücken mit hohem Risiko")
}

# Abschnitt für doctor: osv-scanner und Freigaben
doctor_vulns() {
  local n v
  echo "== Sicherheit"
  n="$(vuln_lockfiles | wc -l | tr -d ' ')"
  if command -v osv-scanner >/dev/null 2>&1; then
    v="$(osv_version)"
    if [ -n "$v" ] && [ "${v%%.*}" -lt 2 ]; then d_warn "osv-scanner $v ist zu alt, nötig ist Version 2"
    else d_ok "osv-scanner ${v:-?} ($n Lockfiles)"; fi
  elif [ "$n" -gt 0 ]; then
    d_warn "osv-scanner fehlt, $n Lockfiles bleiben lokal ungeprüft (Manjaro: sudo pacman -S osv-scanner; Windows: winget install Google.OSVScanner)"
  else
    echo "   osv-scanner: nicht nötig (keine Lockfiles)"
  fi
  [ -f osv-scanner.toml ] && echo "   Freigaben in osv-scanner.toml: $(grep -c '^[[:space:]]*\[\[[[:space:]]*IgnoredVulns' osv-scanner.toml)"
  skip vulns && d_warn "SKIP=vulns: Lücken in Abhängigkeiten werden nicht geprüft"
  return 0
}

# ---------- Rust ----------
rust() { # $1 = Ordner mit Cargo.toml
  need cargo || return
  local m="$1/Cargo.toml" locked=() ws=() lcov
  [ -f "$1/Cargo.lock" ] && locked=(--locked)
  grep -q '^\[workspace\]' "$m" && ws=(--workspace)
  skip fmt || run cargo fmt --manifest-path "$m" --all -- --check
  skip clippy || run cargo clippy --manifest-path "$m" "${ws[@]}" --all-targets "${locked[@]}" -- -D warnings
  [ "$mode" != lint ] && ! skip test || return 0
  if cov_on; then
    if cargo llvm-cov --version >/dev/null 2>&1; then
      lcov="$(cov_new).info"
      run cargo llvm-cov --manifest-path "$m" "${ws[@]}" "${locked[@]}" --lcov --output-path "$lcov" &&
        cov_record "$(cov_key "$1" rust)" "$lcov"
    else
      fail "cargo-llvm-cov fehlt für die Abdeckung (cargo install cargo-llvm-cov)"
    fi
  else
    run cargo test --manifest-path "$m" "${ws[@]}" "${locked[@]}" -q
  fi
}

# ---------- Node (JavaScript, TypeScript, Web-Teil von Tauri) ----------
has_npm_script() { node -e "process.exit(require('./package.json').scripts?.['$1'] ? 0 : 1)" 2>/dev/null; }
has_dep() { node -e "const p=require('./package.json');process.exit({...p.dependencies,...p.devDependencies}['$1'] ? 0 : 1)" 2>/dev/null; }
has_prettier_config() {
  compgen -G '.prettierrc*' >/dev/null || compgen -G 'prettier.config.*' >/dev/null ||
    node -e "process.exit(require('./package.json').prettier ? 0 : 1)" 2>/dev/null
}
has_eslint_config() { compgen -G 'eslint.config.*' >/dev/null || compgen -G '.eslintrc*' >/dev/null; }

node_pkg() {
  need npm || return
  if [ ! -d node_modules ]; then
    if [ -f package-lock.json ]; then run npm ci --no-audit --no-fund; else run npm install --no-audit --no-fund; fi
  fi
  if ! skip eslint; then
    if has_npm_script lint; then
      run npm run lint
    elif has_eslint_config; then
      run npx --no-install eslint --max-warnings 0 .
    else
      fail "ESLint ist nicht eingerichtet (eslint.config.js fehlt)"
    fi
  fi
  if ! skip prettier && has_prettier_config; then
    run npx --no-install prettier --check .
  fi
  if [ -f tsconfig.json ] && ! skip tsc; then
    local tsc_cfg
    for tsc_cfg in tsconfig*.json; do
      # Weitere tsconfig*.json nur mit eigenen Dateien (include/files); reine Basis-Konfigurationen
      # und Sammel-Konfigurationen ("files": []) prüft tsc über die einzelnen Projekte
      if [ "$tsc_cfg" != tsconfig.json ]; then
        grep -qE '"(include|files)"[[:space:]]*:' "$tsc_cfg" || continue
      fi
      grep -qE '"files"[[:space:]]*:[[:space:]]*\[[[:space:]]*\]' "$tsc_cfg" && continue
      # --incremental/--composite false: tsc schreibt sonst trotz --noEmit eine .tsbuildinfo
      run npx --no-install tsc --noEmit -p "$tsc_cfg" --incremental false --composite false
    done
  fi
  [ "$mode" != lint ] && ! skip test || return 0
  local dir
  if cov_on && has_npm_script test && has_dep vitest; then
    dir="$(cov_new)"
    run npm test -- --coverage.enabled=true --coverage.reporter=lcov --coverage.reportsDirectory="$dir" &&
      cov_record "$(cov_key . node)" "$dir/lcov.info"
  elif cov_on && has_npm_script test && has_dep jest; then
    dir="$(cov_new)"
    run npm test -- --coverage --coverageReporters=lcovonly --coverageDirectory="$dir" &&
      cov_record "$(cov_key . node)" "$dir/lcov.info"
  elif cov_on && has_npm_script test && has_dep c8; then
    # c8 misst um jeden Testbefehl herum (node:test, tsx --test, mocha …)
    dir="$(cov_new)"
    run npx --no-install c8 --reporter=lcovonly --reports-dir="$dir" --temp-directory="$dir/tmp" npm test &&
      cov_record "$(cov_key . node)" "$dir/lcov.info"
  else
    cov_on && has_npm_script test && echo "   Abdeckung $(cov_key . node): nicht messbar (weder vitest, jest noch c8)"
    run npm test --if-present
  fi
}

# ---------- Python ----------
python_pkg() {
  if ! skip ruff && need ruff; then
    run ruff check .
    run ruff format --check .
  fi
  [ "$mode" != lint ] && ! skip pytest || return 0
  need pytest || return
  local lcov
  if cov_on; then
    lcov="$(cov_new).info"
    run pytest_ok -q --cov=. --cov-report="lcov:$lcov" &&
      cov_record "$(cov_key . python)" "$lcov"
  else
    run pytest_ok -q
  fi
}

# ---------- C# / .NET ----------
# Erkennung: Solution (*.sln, *.slnx) mit C#-Projekten oder *.csproj im Paketordner.
# SKIP: fmt = dotnet format, analyze = Build mit Warnungen als Fehler, test, coverage.
# Vorlagen: templates/dotnet/Directory.Build.props und .editorconfig.

# Solutions mit C#-Projekten im aktuellen Ordner (C++-Solutions mit .vcxproj zählen nicht)
dotnet_slns() {
  local s
  for s in *.sln *.slnx; do
    [ -f "$s" ] && grep -q '\.csproj' "$s" && echo "$s"
  done
}
is_dotnet() { [ -n "$(dotnet_slns)" ] || compgen -G '*.csproj' >/dev/null; }

# Ziel der dotnet-Befehle: die Solution, sonst das einzige Projekt. Mehrdeutig ist ein
# Fehler, denn dotnet bricht dann ab oder prüft das Falsche. Ausgabe: Ziel bzw. Meldung.
dotnet_target() {
  local s=() p=()
  mapfile -t s < <(dotnet_slns)
  if [ "${#s[@]}" -eq 1 ]; then
    echo "${s[0]}"
    return 0
  elif [ "${#s[@]}" -gt 1 ]; then
    echo "mehrere Solutions (${s[*]}): je Paketordner nur eine; weitere in eigene Ordner und in PACKAGES (.quality), nach dotnet sln migrate die alte .sln löschen"
    return 1
  fi
  mapfile -t p < <(compgen -G '*.csproj')
  if [ "${#p[@]}" -eq 1 ]; then
    echo "${p[0]}"
    return 0
  fi
  echo "mehrere Projekte ohne Solution (${p[*]}): Solution anlegen (dotnet new sln, dotnet sln add) oder Projekte einzeln in PACKAGES (.quality)"
  return 1
}

# Dateien im Paket, versioniert oder neu und nicht ignoriert; Muster wie '*.csproj'
dotnet_files() {
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git ls-files -co --exclude-standard -- "$1"
  else
    find . -path "./$1" -not -path '*/bin/*' -not -path '*/obj/*' | sed 's|^\./||'
  fi
}

# Nächste Datei dieses Namens vom aktuellen Ordner aufwärts (so sucht auch dotnet)
dotnet_up() {
  local d="$PWD"
  while :; do
    [ -f "$d/$1" ] && { echo "$d/$1"; return 0; }
    [ "$d" = / ] && return 1
    d="$(dirname "$d")"
  done
}

# Tests über Microsoft.Testing.Platform statt VSTest (global.json, ab .NET 11 auch DOTNET_TEST_RUNNER)
dotnet_mtp() {
  local g r="${DOTNET_TEST_RUNNER:-}"
  r="${r,,}"
  [ "$r" = vstest ] && return 1
  [ "$r" = microsoft.testing.platform ] && return 0
  g="$(dotnet_up global.json)" && grep -q 'Microsoft\.Testing\.Platform' "$g"
}

# Prüf-Bezug in *.csproj, *.props, *.targets und Solutions (Schutzprüfung, is_mixed_config)
GUARD_KEYWORDS+='|Warn|Nullable|Analy[sz]|CodeStyle|CodeAnalysis|RuleSet|Exclude|Suppress|NuGetAudit|RunSettings|IsTestProject|Compile[[:space:]]+Remove|Tests?\.csproj'

# coverlet schreibt je Testprojekt eine coverage.info, Zeilen je Methode (auch doppelt).
# Zusammenführen: je Datei und Zeile die höchste Trefferzahl; generierter Code fällt heraus.
dotnet_lcov() { # Ergebnisordner von dotnet test
  local f covs=()
  while IFS= read -r f; do covs+=("$f"); done < <(find "$1" -name coverage.info 2>/dev/null)
  if [ "${#covs[@]}" -eq 0 ]; then
    echo "   keine coverage.info: fehlt coverlet.collector im Testprojekt? (quality.sh doctor)" >&2
    return 0
  fi
  awk '/^SF:/ { f = substr($0, 4); sub(/\r$/, "", f)
                skip = (f ~ /\.(g|g\.i|[Dd]esigner|generated)\.cs$/ || f ~ /(^|\/)obj\// || index(f, "\\obj\\") > 0)
                if (!skip && !(f in seen)) { seen[f] = 1; files[++nf] = f }
                next }
       /^DA:/ && !skip { split(substr($0, 4), a, ","); n = a[1] + 0; k = f SUBSEP n
                if (!(k in hit)) { hit[k] = 0; nums[f] = nums[f] " " n }
                if (a[2] + 0 > hit[k]) hit[k] = a[2] + 0 }
       END { for (i = 1; i <= nf; i++) {
               f = files[i]; print "SF:" f
               c = split(nums[f], l, " "); for (j = 1; j <= c; j++) print "DA:" l[j] "," hit[f SUBSEP l[j]]
               print "end_of_record" } }' "${covs[@]}"
}

dotnet_pkg() {
  local target dir lcov built=0 locked=() sel=()
  if ! target="$(dotnet_target)"; then
    fail "$target"
    return
  fi
  need dotnet || return
  local -x DOTNET_NOLOGO=1 DOTNET_CLI_TELEMETRY_OPTOUT=1
  # In der CI muss packages.lock.json zum Projekt passen (wie cargo --locked)
  if [ "${CI:-}" = true ] && dotnet_files '*packages.lock.json' | grep -q .; then locked=(--locked-mode); fi
  run dotnet restore "$target" "${locked[@]}" || return
  skip fmt || run dotnet format "$target" --verify-no-changes --no-restore --severity warn
  if ! skip analyze; then
    # --no-incremental: sonst übergeht MSBuild Projekte, deren Ausgaben (mit Warnungen) schon da sind
    if ! run dotnet build "$target" --no-restore --no-incremental -v q -p:TreatWarningsAsErrors=true; then
      [ "$mode" = lint ] || echo "   Tests übersprungen: Build fehlgeschlagen"
      return
    fi
    built=1
  fi
  [ "$mode" != lint ] && ! skip test || return 0
  sel=("$target")
  if dotnet_mtp; then
    case "$target" in *.csproj) sel=(--project "$target") ;; *) sel=(--solution "$target") ;; esac
  fi
  [ "$built" -eq 1 ] && sel+=(--no-build)
  if cov_on && dotnet_mtp; then
    echo "   Abdeckung $(cov_key . dotnet): mit Microsoft.Testing.Platform nicht messbar (coverlet.collector braucht VSTest)"
    run dotnet test "${sel[@]}"
  elif cov_on; then
    dir="$(cov_new)"
    lcov="$dir.info"
    run dotnet test "${sel[@]}" --collect:"XPlat Code Coverage" --results-directory "$dir" \
      -- DataCollectionRunSettings.DataCollectors.DataCollector.Configuration.Format=lcov &&
      dotnet_lcov "$dir" >"$lcov" && cov_record "$(cov_key . dotnet)" "$lcov"
  else
    run dotnet test "${sel[@]}"
  fi
}

# Setup-Prüfung für doctor (nutzt d_ok, d_warn, d_bad aus doctor)
doctor_dotnet() {
  local target g v props projs=() tests=()
  if target="$(dotnet_target)"; then
    echo "   Stack: C# / .NET ($target)"
  else
    echo "   Stack: C# / .NET"
    d_bad "$target"
  fi
  if g="$(dotnet_up global.json)"; then
    v="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$g" | head -n 1)"
    echo "   Vorgabe global.json: SDK ${v:-?}$(sed -n 's/.*"rollForward"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/, rollForward \1/p' "$g" | head -n 1)"
  fi
  if ! command -v dotnet >/dev/null 2>&1; then
    d_bad "dotnet (Manjaro: sudo pacman -S dotnet-sdk; Windows: winget install Microsoft.DotNet.SDK.10)"
  elif v="$(DOTNET_NOLOGO=1 dotnet --version 2>/dev/null)" && [ -n "$v" ]; then
    # dotnet --version löst global.json auf und scheitert, wenn kein SDK dazu passt
    d_ok "dotnet SDK $(printf '%s\n' "$v" | tail -n 1)"
  else
    d_bad "kein .NET SDK passend zu global.json, installiert: $(dotnet --list-sdks 2>/dev/null | awk '{ print $1 }' | tr '\n' ' ')"
  fi
  props="$(dotnet_up Directory.Build.props)" || d_warn "Directory.Build.props fehlt (Vorlage templates/dotnet/, ROLLOUT.md Schritt 4)"
  mapfile -t projs < <(dotnet_files '*.csproj')
  if grep -qs '<Nullable>enable</Nullable>' "${props:-/dev/null}" ||
    { [ "${#projs[@]}" -gt 0 ] && ! grep -L '<Nullable>enable</Nullable>' "${projs[@]}" | grep -q .; }; then
    d_ok "Nullable eingeschaltet"
  else
    d_warn "Nullable nicht in allen Projekten eingeschaltet (<Nullable>enable</Nullable>): Null-Referenzen bleiben ungeprüft"
  fi
  if grep -qs '<TreatWarningsAsErrors>true</TreatWarningsAsErrors>' "${props:-/dev/null}"; then
    d_ok "TreatWarningsAsErrors in Directory.Build.props"
  else
    d_warn "TreatWarningsAsErrors fehlt in Directory.Build.props (Builds in der IDE lassen Warnungen sonst durch)"
  fi
  dotnet_files '*packages.lock.json' | grep -q . ||
    d_warn "keine packages.lock.json (RestorePackagesWithLockFile, dann dotnet restore; Grundlage der Lückenprüfung)"
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1 && ! { git check-ignore -q bin/x && git check-ignore -q obj/x; }; then
    d_warn "bin/ und obj/ fehlen in .gitignore (dotnet new gitignore)"
  fi
  if [ -f "$cov_file" ]; then
    mapfile -t tests < <(grep -lE 'Microsoft\.NET\.Test\.Sdk|IsTestProject>true|MSTest\.Sdk|xunit|NUnit|TUnit' "${projs[@]}" /dev/null 2>/dev/null)
    if [ "${#tests[@]}" -eq 0 ]; then
      d_warn "kein Testprojekt gefunden: Abdeckung ohne Messwerte"
    elif dotnet_mtp; then
      d_warn "Tests über Microsoft.Testing.Platform: Abdeckung mit coverlet.collector nicht messbar"
    elif grep -qs 'coverlet\.collector' "${tests[@]}" "${props:-/dev/null}"; then
      d_ok "coverlet.collector für die Abdeckung"
    else
      d_bad "coverlet.collector für die Abdeckung (dotnet add <Testprojekt> package coverlet.collector)"
    fi
  fi
}

# ---------- Dart und Flutter ----------
# Dart-Dateien des Pakets ohne generierten Code (build_runner: drift, Isar,
# freezed, mockito) und ohne verschachtelte Pakete mit eigener pubspec.yaml
# (die haben einen eigenen PACKAGES-Eintrag oder bleiben bewusst außen vor).
dart_files() {
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    local excl=() sub
    while IFS= read -r sub; do
      excl+=(":!:$(dirname "$sub")/**")
    done < <(git ls-files -- '*/pubspec.yaml')
    git ls-files -- '*.dart' ':!:*.g.dart' ':!:*.freezed.dart' ':!:*.mocks.dart' "${excl[@]}"
  else
    find . -name '*.dart' -not -name '*.g.dart' -not -name '*.freezed.dart' -not -name '*.mocks.dart' \
      -not -path '*/.dart_tool/*' -not -path '*/build/*' | sed 's|^\./||'
  fi
}

dart_pkg() {
  local tool=dart files=() lcov
  grep -q 'sdk: *flutter' pubspec.yaml && tool=flutter
  need "$tool" || return
  [ -f .dart_tool/package_config.json ] || run "$tool" pub get
  if ! skip fmt; then
    mapfile -t files < <(dart_files)
    [ "${#files[@]}" -gt 0 ] && run dart format --output=none --set-exit-if-changed "${files[@]}"
  fi
  if ! skip analyze; then
    if [ "$tool" = flutter ]; then run flutter analyze; else run dart analyze --fatal-infos; fi
  fi
  [ "$mode" != lint ] && ! skip test && [ -d test ] || return 0
  # shellcheck disable=SC2086  # FLUTTER_TEST_ARGS soll an Leerzeichen getrennt werden
  if cov_on && [ "$tool" = flutter ]; then
    lcov="$(cov_new).info"
    run flutter test -r failures-only --coverage --coverage-path "$lcov" $FLUTTER_TEST_ARGS &&
      cov_record "$(cov_key . dart)" "$lcov"
  else
    cov_on && echo "   Abdeckung $(cov_key . dart): für reine Dart-Pakete nicht gemessen"
    run "$tool" test -r failures-only $FLUTTER_TEST_ARGS
  fi
}

# ---------- C und C++ (CMake) ----------
# Build-Ordner im Paket (wiederverwendet, gehören in .gitignore):
#   build/quality       lint: Debug, Warnungen als Fehler, compile_commands.json für clang-tidy
#   build/quality-test  all:  zusätzlich AddressSanitizer, UBSan, LeakSanitizer und Abdeckung
cpp_os() { # QUALITY_OS nur für tests/selftest.sh
  case "${QUALITY_OS:-$(uname -s)}" in
    MINGW* | MSYS* | CYGWIN*) echo windows ;;
    Darwin) echo mac ;;
    *) echo linux ;;
  esac
}
# Versionierte C/C++-Dateien des Pakets, ohne Fremdcode und Build-Ordner
cpp_files() {
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git ls-files -- '*.c' '*.cc' '*.cpp' '*.cxx' '*.h' '*.hh' '*.hpp' '*.hxx'
  else
    find . -type f | sed 's|^\./||' | grep -E '\.(c|cc|cpp|cxx|h|hh|hpp|hxx)$'
  fi | grep -vE '(^|/)(third_party|vendor|external|build)/'
}
# Konfigurationsdatei im Paket oder darüber bis zum Projektstamm (so suchen auch die Werkzeuge)
cpp_cfg() {
  local d="$PWD" m
  while :; do
    for m in "$@"; do [ -f "$d/$m" ] && return 0; done
    [ "$d" = "$root" ] || [ "$d" = / ] && return 1
    d="$(dirname "$d")"
  done
}
# Compiler: CC/CXX aus der Umgebung, sonst clang (nicht unter Windows), sonst Vorgabe von CMake
cpp_compilers() {
  cc="${CC:-}" cxx="${CXX:-}"
  if [ -z "$cc$cxx" ] && [ "$(cpp_os)" != windows ] && command -v clang >/dev/null 2>&1 &&
    command -v clang++ >/dev/null 2>&1; then
    cc=clang cxx=clang++
  fi
}
cpp_is_clang() { "${cc:-cc}" --version 2>/dev/null | grep -q clang && "${cxx:-c++}" --version 2>/dev/null | grep -q clang; }
cpp_srcdir() { sed -n 's/^CMAKE_HOME_DIRECTORY:INTERNAL=//p' "$1/CMakeCache.txt"; }
cpp_re() { sed 's/[][\.*^$+?(){}|]/\\&/g'; }
cpp_nproc() { nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2; }

# clang-tidy über die Übersetzungseinheiten aus compile_commands.json, die versionierte
# Projektdateien sind; Header nur aus den Projektordnern (nicht build/, third_party/ …)
clang_tidy() { # Build-Ordner
  local src dirs hf sep files=()
  src="$(cpp_srcdir "$1")"
  mapfile -t files < <(cpp_files | awk -v src="$src/" -v db="$1/compile_commands.json" '
    { want[$0] = 1 }
    END {
      while ((getline l < db) > 0) {
        if (l !~ /^[ \t]*"file":/) continue
        sub(/^[ \t]*"file":[ \t]*"/, "", l); sub(/",?[ \t\r]*$/, "", l)
        if (index(l, src) != 1) continue
        r = substr(l, length(src) + 1)
        if ((r in want) && !(r in seen)) { seen[r] = 1; print r }
      }
    }')
  if [ "${#files[@]}" -eq 0 ]; then
    echo "keine Übersetzungseinheiten gefunden"
    return 0
  fi
  dirs="$(cpp_files | awk -F/ 'NF > 1 { print $1 }' | sort -u | cpp_re | paste -sd '|' -)"
  # Trenner / oder \ (unter Windows hängt clang Header mit \ an)
  sep='[/\\]'
  hf="^$(printf '%s' "$src" | cpp_re | sed 's|/|[/\\\\]|g')$sep(${dirs:+($dirs)$sep|}[^/\\]*$)"
  printf '%s\0' "${files[@]}" |
    xargs -0 -P "$(cpp_nproc)" -n 4 clang-tidy -p "$1" --quiet --warnings-as-errors='*' --header-filter="$hf" 2>&1 |
    grep -vE '^\[[0-9]+/[0-9]+\] Processing file '
  return "${PIPESTATUS[1]}"
}

# Abdeckung: Profile zusammenführen, instrumentierte Programme finden, LCOV nur für
# versionierte Projektdateien ohne Tests (Ordner test/, tests/; *_test.*, test_*.*, *Test.*)
cpp_coverage() { # Build-Ordner, Profil-Ordner, LCOV-Ziel
  local src objs=() o
  : >"$3"
  if ! compgen -G "$2/*.profraw" >/dev/null; then
    echo "keine Profildaten (keine Tests registriert?)"
    return 0
  fi
  llvm-profdata merge -sparse "$2"/*.profraw -o "$2/all.profdata" || return
  while IFS= read -r o; do objs+=(-object "$o"); done < <(
    find "$1" -type f \( -perm -u+x -o -name '*.so' -o -name '*.so.*' -o -name '*.dylib' -o -name '*.dll' \) \
      -not -path '*/CMakeFiles/*' -exec grep -lE '__llvm_covmap|lcovmap' {} +
  )
  [ "${#objs[@]}" -gt 0 ] || { echo "keine instrumentierten Programme in $1"; return 1; }
  llvm-cov export -format=lcov -instr-profile="$2/all.profdata" "${objs[@]:1}" >"$3.raw" || return
  src="$(cpp_srcdir "$1")"
  cpp_files | awk -v src="$src/" -v raw="$3.raw" '
    $0 !~ "(^|/)(tests?|unittests?)/" && $0 !~ "(^|/)tests?[_.][^/]*$" && $0 !~ "([_.]tests?|Tests?)\\.[a-z]+$" { want[$0] = 1 }
    END {
      while ((getline l < raw) > 0) {
        if (l ~ /^SF:/) { f = substr(l, 4); keep = index(f, src) == 1 && ((substr(f, length(src) + 1)) in want) }
        if (keep) print l
      }
    }' >"$3"
}

cpp_pkg() {
  need cmake || return
  local files=() cc cxx os bdir=build/quality flags="" san=0 cov=0 args=() prof lcov
  os="$(cpp_os)"
  if ! skip fmt; then
    mapfile -t files < <(cpp_files)
    if [ "${#files[@]}" -eq 0 ]; then
      :
    elif ! cpp_cfg .clang-format _clang-format; then
      fail "clang-format ist nicht eingerichtet (.clang-format fehlt, Vorlage templates/cpp/)"
    elif need clang-format; then
      run clang-format --dry-run -Werror "${files[@]}"
    fi
  fi
  need ninja || return
  cpp_compilers
  if [ "$mode" != lint ] && ! skip test; then
    bdir=build/quality-test
    case "$os" in
      windows) echo "   Sanitizer: unter Windows übersprungen (MinGW/MSVC), sie laufen unter Linux und in der CI" ;;
      *) san=1 flags="-fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all" ;;
    esac
    if cov_on; then
      if ! cpp_is_clang; then
        fail "Abdeckung für C/C++ braucht clang (CC/CXX, siehe QUALITY.md)"
      elif need llvm-profdata && need llvm-cov; then
        cov=1 flags="${flags:+$flags }-fprofile-instr-generate -fcoverage-mapping"
      fi
    fi
  fi
  args=(-S . -B "$bdir" -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    -DCMAKE_COMPILE_WARNING_AS_ERROR=ON)
  [ -n "$cc" ] && args+=("-DCMAKE_C_COMPILER=$cc")
  [ -n "$cxx" ] && args+=("-DCMAKE_CXX_COMPILER=$cxx")
  # Ohne eigene Flags die Vorgabe von CMake wiederherstellen (sonst bleiben alte im Cache)
  if [ -n "$flags" ]; then args+=("-DCMAKE_C_FLAGS=$flags" "-DCMAKE_CXX_FLAGS=$flags")
  else args+=(-UCMAKE_C_FLAGS -UCMAKE_CXX_FLAGS); fi
  run cmake "${args[@]}" || return
  run cmake --build "$bdir" || return
  if [ "$san" -eq 1 ] && ! grep -q -- '-fsanitize=address' "$bdir/compile_commands.json"; then
    fail "Sanitizer-Flags kommen nicht an: überschreibt CMakeLists.txt CMAKE_CXX_FLAGS?"
  fi
  if ! skip tidy; then
    if ! cpp_cfg .clang-tidy; then
      fail "clang-tidy ist nicht eingerichtet (.clang-tidy fehlt, Vorlage templates/cpp/)"
    elif need clang-tidy; then
      run clang_tidy "$bdir"
    fi
  fi
  if ! skip cppcheck && command -v cppcheck >/dev/null 2>&1; then
    local src
    src="$(cpp_srcdir "$bdir")"
    run cppcheck --project="$bdir/compile_commands.json" --quiet --error-exitcode=1 --inline-suppr \
      --enable=warning,performance,portability --suppress=missingIncludeSystem \
      -i"$src/build/" -i"$src/third_party/" -i"$src/vendor/" -i"$src/external/"
  fi
  [ "$mode" != lint ] && ! skip test || return 0
  local asan="halt_on_error=1:detect_stack_use_after_return=1:check_initialization_order=1"
  # Jeder Befund der Sanitizer beendet den Test mit Fehler; Lecks meldet LeakSanitizer nur unter Linux
  [ "$os" = linux ] && asan="detect_leaks=1:$asan"
  prof="$(cov_new)"
  mkdir -p "$prof"
  ASAN_OPTIONS="$asan" UBSAN_OPTIONS="halt_on_error=1:print_stacktrace=1" LLVM_PROFILE_FILE="$prof/%p.profraw" \
    run ctest --test-dir "$bdir" --output-on-failure -j "$(cpp_nproc)" || return
  if [ "$cov" -eq 1 ]; then
    lcov="$(cov_new).info"
    run cpp_coverage "$bdir" "$prof" "$lcov" && cov_record "$(cov_key . cpp)" "$lcov"
  fi
}

# Abschnitt für doctor (nutzt d_ok, d_warn, d_bad und any aus doctor); 1 = kein CMake-Projekt
cpp_doctor() {
  local cc cxx t v
  if [ ! -f CMakeLists.txt ]; then
    if [ "$any" -eq 0 ] && [ -n "$(cpp_files | head -n 1)" ]; then
      d_warn "C/C++ ohne CMakeLists.txt: andere Build-Systeme (make, meson …) prüft quality.sh nicht"
    fi
    return 1
  fi
  echo "   Stack: C/C++ (CMake)"
  cpp_tool() { # Befehl, Hinweis
    if command -v "$1" >/dev/null 2>&1; then
      d_ok "$1 $("$1" --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n 1)"
    else
      d_bad "$1 ($2)"
    fi
  }
  cpp_tool cmake "Manjaro: sudo pacman -S cmake; Windows: winget install Kitware.CMake"
  v="$(cmake --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+' | head -n 1)"
  if [ -n "$v" ] && awk -v v="$v" 'BEGIN { split(v, a, "."); exit !(a[1] < 3 || (a[1] == 3 && a[2] < 24)) }'; then
    d_warn "CMake $v: Warnungen als Fehler (CMAKE_COMPILE_WARNING_AS_ERROR) erst ab 3.24"
  fi
  cpp_tool ninja "Manjaro: sudo pacman -S ninja; Windows: winget install Ninja-build.Ninja"
  cpp_compilers
  for t in "${cc:-cc}" "${cxx:-c++}"; do
    if command -v "$t" >/dev/null 2>&1; then d_ok "Compiler $t: $("$t" --version 2>/dev/null | head -n 1)"
    else d_bad "Compiler $t (CC/CXX prüfen)"; fi
  done
  cpp_tool clang-format "Manjaro: sudo pacman -S clang; Windows: winget install LLVM.LLVM"
  cpp_tool clang-tidy "Manjaro: sudo pacman -S clang; Windows: winget install LLVM.LLVM"
  command -v cppcheck >/dev/null 2>&1 && d_ok "cppcheck $(cppcheck --version 2>/dev/null | awk '{ print $2 }') (optional)"
  for t in .clang-format .clang-tidy; do
    if cpp_cfg "$t" || { [ "$t" = .clang-format ] && cpp_cfg _clang-format; }; then d_ok "$t vorhanden"
    else d_bad "$t fehlt: nicht eingerichtet (Vorlage templates/cpp/, ROLLOUT.md Schritt 4)"; fi
  done
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    if ! git ls-files -- '*CMakeLists.txt' '*.cmake' | xargs grep -qsE -- '-Wall|/W[34]' 2>/dev/null; then
      d_warn "keine Compiler-Warnungen eingeschaltet (-Wall -Wextra bzw. /W4 in CMakeLists.txt)"
    fi
    git check-ignore -q build/quality || d_warn "build/ fehlt in .gitignore (Build-Ordner build/quality*)"
  fi
  [ "$(cpp_os)" = windows ] && d_warn "Sanitizer laufen unter Windows nicht; quality.sh all überspringt sie, die CI prüft"
  if [ -f "$cov_file" ]; then
    cpp_is_clang && d_ok "Abdeckung: Compiler ist clang" || d_bad "Abdeckung braucht clang (CC/CXX)"
    cpp_tool llvm-profdata "Manjaro: sudo pacman -S llvm; Windows: winget install LLVM.LLVM"
    cpp_tool llvm-cov "Manjaro: sudo pacman -S llvm; Windows: winget install LLVM.LLVM"
  fi
  return 0
}

# ---------- Setup prüfen (doctor) ----------
# Prüft, ob Werkzeuge, Versionen, Pfade und Einrichtung zum Projekt passen. Ändert nichts.
doctor() {
  local bad=0 warn=0 d v want master hp cv=
  d_ok() { echo "   ok: $*"; }
  d_warn() { echo "   Achtung: $*"; warn=$((warn + 1)); }
  d_bad() { echo "   FEHLT: $*"; bad=$((bad + 1)); }
  d_tool() { # Befehl, Hinweis
    if command -v "$1" >/dev/null 2>&1; then
      d_ok "$1 $("$1" --version 2>/dev/null | head -n 1)"
    else
      d_bad "$1 ($2)"
    fi
  }
  major() { printf '%s' "$1" | sed -E 's/^[^0-9]*([0-9]+).*/\1/'; }
  # Strenges Profil (QUALITY.md §9) eines Crates: Lints in Cargo.toml, #![deny(...)] im
  # Crate-Stamm (Repo-Standard §5.4) oder [lints] workspace = true bei Workspace-Lints
  rust_strict() { # Ordner mit Cargo.toml, Workspace hat Lints (0/1)
    grep -qs 'unwrap_used' "$1/Cargo.toml" "$1/src/main.rs" "$1/src/lib.rs" && return 0
    [ "$2" = 1 ] && awk '/^\[/ { s = ($0 ~ /^\[lints\]/) }
      s && /^workspace *= *true/ { f = 1 } /^lints *(\.|=).*workspace *= *true/ { f = 1 }
      END { exit !f }' "$1/Cargo.toml"
  }
  d_rust_strict() { # Ordner mit Cargo.toml
    local d="$1" ws=0 pat m miss=() where=""
    [ "$d" != . ] && where=" ($d)"
    if ! grep -q '^\[workspace\]' "$d/Cargo.toml"; then
      if rust_strict "$d" 0; then d_ok "strenges Rust-Profil aktiv$where"
      else d_warn "strenges Rust-Profil fehlt$where: unwrap_used, unsafe_code (ROLLOUT.md Schritt 4, Strenges Profil)"; fi
      return
    fi
    grep -q '^\[workspace\.lints' "$d/Cargo.toml" && grep -q 'unwrap_used' "$d/Cargo.toml" && ws=1
    grep -q '^\[package\]' "$d/Cargo.toml" && { rust_strict "$d" "$ws" || miss+=(.); }
    for pat in $(awk '/^\[/ { w = ($0 == "[workspace]") } w && /^members/ { m = 1 } m { print } m && /\]/ { exit }' "$d/Cargo.toml" |
      grep -oE '"[^"]+"' | tr -d '"'); do
      for m in "$d"/$pat; do
        [ -f "$m/Cargo.toml" ] && { rust_strict "$m" "$ws" || miss+=("${m#"$d"/}"); }
      done
    done
    if [ "${#miss[@]}" -eq 0 ]; then d_ok "strenges Rust-Profil aktiv$where, alle Workspace-Mitglieder"
    elif [ "$ws" = 1 ]; then d_warn "Workspace-Mitglieder ohne [lints] workspace = true, strenges Profil gilt dort nicht: ${miss[*]}"
    else d_warn "strenges Rust-Profil fehlt$where: [workspace.lints] mit unwrap_used (ROLLOUT.md Schritt 4, Strenges Profil)"; fi
  }
  # Tauri: ohne CSP hätte eingeschleustes Skript Zugriff auf alle freigegebenen Befehle
  d_tauri_csp() { # Pfad zu tauri.conf.json
    local csp
    # Tauri 2: app.security.csp, Tauri 1: tauri.security.csp; ohne node (oder bei Lesefehler) grob per grep
    csp="$(node -e 'const c = require(require("path").resolve(process.argv[1]));
      const s = ((c.app || c.tauri || {}).security || {}).csp;
      process.stdout.write(s == null ? "" : typeof s === "string" ? s : JSON.stringify(s))' "$1" 2>/dev/null)" ||
      csp="$(grep -oE '"csp"[[:space:]]*:[[:space:]]*("[^"]+"|\{)' "$1")"
    if [ -n "$csp" ]; then d_ok "Tauri: Content-Security-Policy gesetzt"
    else d_warn "Tauri: keine Content-Security-Policy (app.security.csp fehlt oder null in $1, QUALITY.md §9)"; fi
  }

  echo "== Vorlage und Einrichtung"
  echo "   Projekt: $root"
  echo "   Laufendes Skript: $(readlink -f "${BASH_SOURCE[0]}" 2>/dev/null || echo "$0") ($QUALITY_VERSION)"
  if [ -f "$root/scripts/quality.sh" ]; then
    cv="$(sed -n 's/^QUALITY_VERSION=//p' "$root/scripts/quality.sh" | head -n 1)"
    echo "   Kopie im Projekt: scripts/quality.sh (${cv:-?})"
  else
    d_warn "Kopie fehlt: Projekt noch nicht umgestellt (scripts/quality.sh, ROLLOUT.md)"
  fi
  master="$HOME/.local/bin/quality.sh"
  if [ -f "$master" ]; then
    v="$(bash "$master" --version 2>/dev/null | awk '{ print $2 }')"
    if [ -z "$cv" ] || [ "$v" = "$cv" ]; then d_ok "Vorlage ~/.local/bin/quality.sh ($v)"
    else d_warn "Vorlage hat Version ${v:-?}, Kopie $cv: Kopie nachziehen (ROLLOUT.md, Update)"; fi
  else
    d_warn "Vorlage nicht installiert (~/.local/bin/quality.sh fehlt): install.sh ausführen"
  fi
  hp="$(git config --global core.hooksPath 2>/dev/null)"
  hp="${hp/#\~/$HOME}"
  if [ -n "$hp" ] && [ -f "$hp/pre-commit" ] && grep -q 'quality' "$hp/pre-commit" 2>/dev/null; then
    d_ok "pre-commit (Stufe 3) aktiv über $hp"
  else
    d_warn "pre-commit (Stufe 3) nicht aktiv: install.sh ausführen"
  fi
  if grep -q 'quality-hook.sh' "$HOME/.claude/settings.json" 2>/dev/null; then
    d_ok "Claude-Code-Hook (Stufe 1) eingetragen"
  else
    d_warn "Claude-Code-Hook (Stufe 1) fehlt in ~/.claude/settings.json (nur für Claude Code nötig)"
  fi
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    if [ "$(git check-attr eol -- scripts/quality.sh | awk '{ print $NF }')" = lf ]; then
      d_ok ".gitattributes: Shell-Skripte mit LF"
    else
      d_warn ".gitattributes: *.sh text eol=lf fehlt (unter Windows bricht Bash sonst ab)"
    fi
    v="$(git ls-files -s scripts/quality.sh | awk '{ print $1 }')"
    if [ -z "$v" ]; then d_warn "scripts/quality.sh ist noch nicht versioniert"
    elif [ "$v" != 100755 ]; then d_warn "scripts/quality.sh ohne Ausführbar-Bit im Repo (git update-index --chmod=+x)"; fi
  fi

  for pkg in "${PACKAGES[@]}"; do
    echo "== Paket $pkg"
    if ! cd "$root/$pkg" 2>/dev/null; then d_bad "Ordner fehlt (PACKAGES in .quality prüfen)"; continue; fi
    local any=0
    if [ -f Cargo.toml ] || [ -f src-tauri/Cargo.toml ]; then
      any=1
      echo "   Stack: Rust$([ -f src-tauri/Cargo.toml ] && echo " (Tauri)")"
      d_tool cargo "rustup.rs"
      d_tool rustfmt "rustup component add rustfmt"
      cargo clippy --version >/dev/null 2>&1 && d_ok "clippy" || d_bad "clippy (rustup component add clippy)"
      for f in rust-toolchain.toml src-tauri/rust-toolchain.toml; do
        [ -f "$f" ] && echo "   Vorgabe $f: $(grep -E '^(channel|components)' "$f" | tr '\n' ' ')"
      done
      if [ -f "$cov_file" ]; then
        cargo llvm-cov --version >/dev/null 2>&1 && d_ok "cargo-llvm-cov" || d_bad "cargo-llvm-cov für die Abdeckung (cargo install cargo-llvm-cov)"
      fi
      for f in . src-tauri; do
        [ -f "$f/Cargo.toml" ] && d_rust_strict "$f"
      done
      if [ -f src-tauri/tauri.conf.json ]; then
        d_tauri_csp src-tauri/tauri.conf.json
      elif [ -f src-tauri/Cargo.toml ]; then
        echo "   CSP nicht geprüft: nur src-tauri/tauri.conf.json wird gelesen"
      fi
    fi
    if [ -f package.json ]; then
      any=1
      echo "   Stack: $([ -f tsconfig.json ] && echo TypeScript || echo JavaScript)"
      d_tool node "Node LTS"
      d_tool npm "kommt mit Node"
      if npm --version 2>&1 | grep -q 'does not support Node.js'; then
        d_warn "npm $(npm --version 2>/dev/null) passt nicht zu Node $(node --version), npm verlangt Node $(npm --version 2>&1 | sed -n 's/.*supports the following node versions: \(.*\)\. You can.*/\1/p' | head -n 1)"
      fi
      want=""
      [ -f .nvmrc ] && want="$(cat .nvmrc)"
      [ -z "$want" ] && want="$(node -e "console.log(require('./package.json').engines?.node || '')" 2>/dev/null)"
      if [ -n "$want" ]; then
        if [ "$(major "$want")" = "$(major "$(node --version 2>/dev/null)")" ] || [[ "$want" == *">"* ]]; then
          d_ok "Node-Vorgabe $want, installiert $(node --version 2>/dev/null)"
        else
          d_warn "Node-Vorgabe $want, installiert $(node --version 2>/dev/null)"
        fi
      fi
      [ -d node_modules ] || d_warn "node_modules fehlt (quality.sh führt npm ci aus)"
      if has_npm_script lint || has_eslint_config; then d_ok "ESLint eingerichtet"; else d_bad "ESLint nicht eingerichtet (ROLLOUT.md Schritt 4)"; fi
      if [ -f tsconfig.json ] && [ -x node_modules/.bin/tsc ]; then
        local listed f2 dir2
        listed="$(for f2 in tsconfig*.json; do node_modules/.bin/tsc -p "$f2" --listFilesOnly 2>/dev/null; done)"
        for dir2 in test tests tools scripts e2e; do
          [ -n "$(git ls-files -- "$dir2/*.ts" "$dir2/*.tsx" "$dir2/*.mts" 2>/dev/null | head -n 1)" ] || continue
          if ! printf '%s\n' "$listed" | grep -q "/$dir2/"; then
            d_warn "TypeScript in $dir2/ steht in keiner tsconfig: Typen dort ungeprüft (tsc prüft nur include)"
          fi
        done
      fi
      # Strenges Profil (QUALITY.md §9); tsc --showConfig löst auch extends auf
      if [ -f tsconfig.json ]; then
        v="$(cat tsconfig*.json 2>/dev/null; [ -x node_modules/.bin/tsc ] && node_modules/.bin/tsc --showConfig 2>/dev/null)"
        if grep -qE '"strict": *true' <<<"$v" && grep -qE '"noUncheckedIndexedAccess": *true' <<<"$v"; then
          d_ok "strenge tsconfig (strict, noUncheckedIndexedAccess)"
        else
          d_warn "strenge tsconfig fehlt: strict, noUncheckedIndexedAccess (ROLLOUT.md Schritt 4, Strenges Profil)"
        fi
        if grep -qsE 'no-floating-promises|(recommended|strict)TypeChecked' eslint.config.* .eslintrc*; then
          d_ok "ESLint typgeprüft (no-floating-promises)"
        elif has_eslint_config; then
          d_warn "ESLint ohne typgeprüfte Regeln wie no-floating-promises (ROLLOUT.md Schritt 4, Strenges Profil)"
        fi
      elif has_eslint_config; then
        if grep -qs 'no-new-func' eslint.config.* .eslintrc*; then d_ok "ESLint mit no-eval, no-new-func"
        else d_warn "ESLint ohne no-eval, no-implied-eval, no-new-func (ROLLOUT.md Schritt 4, Strenges Profil)"; fi
      fi
      if [ -f "$cov_file" ] && has_npm_script test; then
        if has_dep vitest || has_dep jest || has_dep c8; then d_ok "Abdeckung messbar"
        else d_warn "Abdeckung nicht messbar: vitest, jest oder c8 fehlt"; fi
      fi
    fi
    if [ -f pyproject.toml ]; then
      any=1
      echo "   Stack: Python"
      local py=""
      for c in python3 python py; do command -v "$c" >/dev/null 2>&1 && "$c" -c '' </dev/null 2>/dev/null && { py="$c"; break; }; done
      if [ -n "$py" ]; then d_ok "Python $("$py" --version 2>&1 | awk '{ print $2 }')"; else d_bad "Python"; fi
      want="$(sed -n 's/^requires-python *= *"\(.*\)"/\1/p' pyproject.toml | head -n 1)"
      [ -n "$want" ] && echo "   Vorgabe requires-python: $want"
      d_tool ruff "Manjaro: sudo pacman -S ruff; Windows: pip install ruff"
      d_tool pytest "Manjaro: sudo pacman -S python-pytest; Windows: pip install pytest"
      if [ -f "$cov_file" ] && [ -n "$py" ]; then
        "$py" -c 'import pytest_cov' 2>/dev/null && d_ok "pytest-cov" || d_bad "pytest-cov für die Abdeckung"
      fi
      # Strenges Profil (QUALITY.md §9): ruff mit Bandit-Regeln, pytest mit ResourceWarning als Fehler
      v=""
      awk '/^\[/ { s = ($0 ~ /^\[(tool\.ruff\.)?lint\]/) } s && /"S"/ { f = 1 } END { exit !f }' \
        pyproject.toml $(ls ruff.toml .ruff.toml 2>/dev/null) || v="ruff-Regeln \"S\""
      grep -qs 'error::ResourceWarning' pyproject.toml pytest.ini || v="${v:+$v, }pytest filterwarnings error::ResourceWarning"
      if [ -z "$v" ]; then d_ok "strenges Python-Profil (ruff S, ResourceWarning)"
      else d_warn "strenges Python-Profil fehlt: $v (ROLLOUT.md Schritt 4, Strenges Profil)"; fi
    fi
    if [ -f pubspec.yaml ]; then
      any=1
      if grep -q 'sdk: *flutter' pubspec.yaml; then
        echo "   Stack: Flutter"
        command -v flutter >/dev/null && d_ok "$(flutter --version 2>/dev/null | head -n 1)" || d_bad "flutter (flutter.dev)"
      else
        echo "   Stack: Dart"
      fi
      d_tool dart "kommt mit Flutter"
      want="$(sed -n 's/^ *sdk: *\(.*\)/\1/p' pubspec.yaml | grep -v flutter | head -n 1)"
      [ -n "$want" ] && echo "   Vorgabe Dart-SDK: $want"
      # Strenges Profil (QUALITY.md §9): Analyzer-Regeln und Leak-Tracker in Widget-Tests
      if grep -qsE '^ *strict-casts: *true' analysis_options.yaml; then d_ok "strenge analysis_options (strict-casts)"
      else d_warn "strenge analysis_options fehlen: strict-casts, unawaited_futures … (ROLLOUT.md Schritt 4, Strenges Profil)"; fi
      if grep -q 'sdk: *flutter' pubspec.yaml && [ -d test ]; then
        if grep -qs 'LeakTesting.enable' test/flutter_test_config.dart; then d_ok "Leak-Tracker in test/flutter_test_config.dart"
        else d_warn "Leak-Tracker fehlt: test/flutter_test_config.dart mit LeakTesting.enable() (ROLLOUT.md Schritt 4, Strenges Profil)"; fi
      fi
    fi
    cpp_doctor && any=1
    is_dotnet && { any=1; doctor_dotnet; }
    [ "$any" -eq 1 ] || d_warn "kein Stack erkannt (PACKAGES in .quality oder Markerdateien prüfen)"
  done
  cd "$root" || return
  doctor_vulns

  echo "== CI"
  if compgen -G '.github/workflows/*.y*ml' >/dev/null; then
    if grep -lq 'scripts/quality.sh' .github/workflows/*.y*ml 2>/dev/null; then d_ok "Workflow ruft scripts/quality.sh auf"
    else d_warn "kein Workflow ruft scripts/quality.sh auf (ROLLOUT.md Schritt 7)"; fi
    if grep -nE 'uses: [^ ]+@v[0-9]' .github/workflows/*.y*ml >/dev/null 2>&1; then
      d_warn "Actions nicht per Commit festgenagelt (Repo-Standard R3)"
    fi
  else
    d_warn "keine CI (.github/workflows fehlt)"
  fi

  echo
  if [ "$bad" -gt 0 ]; then
    echo "quality.sh doctor: $bad Probleme, $warn Hinweise"
    return 1
  fi
  if [ "$warn" -gt 0 ]; then
    echo "quality.sh doctor: Setup in Ordnung, $warn Hinweise"
  else
    echo "quality.sh doctor: Setup in Ordnung"
  fi
  return 0
}

# ---------- Ablauf ----------
if [ "$mode" = doctor ]; then
  doctor
  rc=$?
  rm -rf "$logdir"
  exit "$rc"
fi
guard
if [ "$mode" != lint ]; then
  if skip coverage; then echo ">> Abdeckung: per SKIP aus"
  elif [ ! -f "$cov_file" ] && [ "$mode" != coverage-update ]; then
    echo ">> Abdeckung: aus (keine .quality-coverage; einschalten mit scripts/quality.sh coverage-update)"
  elif [ "${CI:-}" != true ] && [ "$COVERAGE_LOCAL" != 1 ] && [ "$mode" != coverage-update ]; then
    echo ">> Abdeckung: nur in der CI (COVERAGE_LOCAL=0)"
  fi
fi
for pkg in "${PACKAGES[@]}"; do
  if ! cd "$root/$pkg" 2>/dev/null; then
    fail "Ordner fehlt"
    continue
  fi
  [ -f Cargo.toml ] && { found=1; rust .; }
  [ -f src-tauri/Cargo.toml ] && { found=1; rust src-tauri; }
  [ -f package.json ] && { found=1; node_pkg; }
  [ -f pyproject.toml ] && { found=1; python_pkg; }
  [ -f pubspec.yaml ] && { found=1; dart_pkg; }
  [ -f CMakeLists.txt ] && { found=1; cpp_pkg; }
  is_dotnet && { found=1; dotnet_pkg; }
done
cd "$root" || exit 1
[ "$mode" = lint ] || vulns
[ "${#failed[@]}" -eq 0 ] && cov_write

# Zusammenfassung auf der CI-Seite (GitHub Actions)
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "### quality.sh $mode"
    [ "${#failed[@]}" -gt 0 ] && printf -- '- ❌ %s\n' "${failed[@]}"
    [ "${#summary[@]}" -gt 0 ] && printf -- '- %s\n' "${summary[@]}"
    [ "${#failed[@]}" -eq 0 ] && echo "- ✅ alles grün"
  } >>"$GITHUB_STEP_SUMMARY" 2>/dev/null
fi

echo
if [ "${#failed[@]}" -gt 0 ]; then
  echo "quality.sh $mode: FEHLGESCHLAGEN"
  printf '  - %s\n' "${failed[@]}"
  echo "Protokolle: $logdir"
  exit 1
fi
rm -rf "$logdir"
if [ "$found" -eq 0 ]; then
  echo "quality.sh: kein Stack erkannt in $root, nichts zu prüfen"
  exit 0
fi
echo "quality.sh $mode: alles grün"
