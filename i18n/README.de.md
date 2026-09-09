<div align="center">
  <img src="../docs/public/icon.png" alt="Lap Logo" width="120" style="border-radius: 20px">
  <h1>Lap – Privater lokaler Fotomanager</h1>
  <h3>Open-Source-Desktop-Fotomanager für macOS, Windows und Linux.</h3>
  <p>
    <a href="https://github.com/julyx10/lap/releases"><img src="https://img.shields.io/github/v/release/julyx10/lap" alt="GitHub release"></a>
    <a href="https://github.com/julyx10/lap/releases"><img src="https://img.shields.io/github/downloads/julyx10/lap/total" alt="GitHub all releases"></a>
    <a href="https://github.com/julyx10/lap/stargazers"><img src="https://img.shields.io/github/stars/julyx10/lap" alt="GitHub stars"></a>
  </p>
</div>

[English](../README.md) | Deutsch | [Français](README.fr.md) | [Español](README.es.md) | [Português](README.pt.md) | [Русский](README.ru.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) 

Lap ist ein quelloffener, lokal orientierter Fotomanager zum Durchsuchen von Familienalben, zum schnellen Finden alter Fotos und zum Offline-Verwalten großer persönlichen Medienbibliotheken.
Es ist eine datenschutzorientierte Alternative zu Cloud-Fotodiensten: kein erzwungener Upload, lokale KI-Suche, ordnerorientierter Workflow und kostenlos nutzbar.

- Website: [https://julyx10.github.io/lap/](https://julyx10.github.io/lap/)
- Demo-Video: [https://youtu.be/RbKqNKhbVUs](https://youtu.be/RbKqNKhbVUs)
- Datenschutz: [PRIVACY.md](../PRIVACY.md)

## Lap herunterladen

Öffnen Sie die Seite der [neuesten Veröffentlichungen](https://github.com/julyx10/lap/releases/latest) und laden Sie die Datei herunter, die Ihrem System entspricht:

| Plattform | Paket | Hinweis |
| :-- | :-- | :-- |
| **macOS (Apple Silicon / Intel)** | `_aarch64.dmg` / `_x64.dmg` | Von Apple notarisiert |
| **Windows 10/11 (x64 / ARM64)** | `_x64_en-US.msi` / `_arm64_en-US.msi` | Nicht signiert — falls SmartScreen den Download blockiert, klicken Sie auf **Trotzdem behalten** |
| **Linux (amd64 / arm64)** | `_amd64.deb` / `_arm64.deb` | Für Debian-basierte Distributionen (Ubuntu, Debian, Linux Mint, etc.) |

### macOS mit Homebrew

```bash
brew tap julyx10/lap
brew install --cask lap
```

## Screenshots

<p align="center">
  <img src="../docs/public/screenshots/lap_library.png" alt="Lokaler Fotobibliotheksmanager Lap" width="900">
  <img src="../docs/public/screenshots/lap_map_view.png" alt="Kartenansicht von Lap" width="900">
</p>

## Warum Lap

- **Lokal zuerst konzipiert**: Ihre Fotos bleiben auf Ihrer eigenen Festplatte, ohne erforderliches Cloud-Konto oder Upload.
- **Kein Bibliotheks-Lock-in**: Arbeiten Sie direkt mit Ihren vorhandenen Ordnern, statt alles in eine geschlossene Datenbank zu importieren.
- **Lokale KI-Werkzeuge**: Suche, Ähnlichkeit, intelligente Tags und Gesichtserkennung laufen lokal auf Ihrem Gerät.
- **Für große Sammlungen gebaut**: Optimiert für flüssiges Durchsuchen und Organisieren von Bibliotheken mit 100.000+ Dateien.
- **Open Source und kostenlos**: Kein Abonnement, kein erzwungenes Ökosystem und Code, den Sie prüfen können.

## Funktionen

- **Flexibles Durchsuchen der Bibliothek** mit Timeline-, Ordner-, Standort-, Kamera-, Objektiv-, Tag-, Favoriten-, Bewertungs-, Motiv- und Gesichtsfiltern.
- **Smart-Alben** speichern regelbasierte Ansichten mit eigener Gruppierung, Sortierung und Reihenfolge.
- **Sammlungen**: Organisieren Sie Dateien in flexiblen Sammlungen, ohne die Originale zu verschieben oder zu duplizieren.
- **Lokale KI-Suche** für Textanfragen, visuelle Ähnlichkeit, Motive, Gesichts-Clustering und optionale mehrsprachige Suche in über 50 Sprachen.
- **Apple Live Photos** erkennt gekoppelte HEIC/MOV-Live-Photos, spielt sie im Viewer ab und hält verknüpfte MOV- und AAE-Seitendateien beim Umbenennen, Verschieben, Kopieren und Löschen zusammen.
- **RAW- + JPEG/HEIC-Paare** gruppieren optional eine RAW-Datei und die gleichnamige JPEG- oder HEIC-Begleitdatei im selben Ordner als ein Element. Die Originale bleiben getrennte Dateien; beim Umbenennen, Verschieben, Kopieren, Einfügen und Löschen werden beide Dateien zusammen behandelt.
- **Ordnerorientierter Workflow** mit mehreren Bibliotheken, Drag-and-drop-Import, Kopieren-und-Einfügen-Import, Dateisystem-Synchronisierung und sicheren Verschiebe-/Kopier-/Löschvorgängen.
- **Auswahl- und Vergleichswerkzeuge** einschließlich eines Vierfach-Bildvergleichs im Viewer.
- **Aufräumwerkzeuge** zum Finden von Duplikaten und zum stapelweisen Verschieben unerwünschter Dateien in den Papierkorb.
- **Integrierte Bearbeitung** für Zuschneiden, Drehen, Spiegeln, Größenänderung und grundlegende Bildanpassungen.
- **Breite Formatunterstützung** für über 60 Foto-, RAW- und Videoformate.

## Metadaten, Sammlungen und das Verschieben von Dateien

Lap arbeitet ordnerorientiert, aber nicht jede in Lap angezeigte Information ist in der Originaldatei eingebettet. Das ist wichtig, wenn Sie dieselben Ordner auch im Finder, Explorer oder einer anderen Foto-App verwalten.

### Was bei der Datei bleibt

- Ihre Originalfotos und -videos bleiben stets normale Dateien in ihren vorhandenen Ordnern.
- Bereits in einer Datei eingebettete Metadaten wie EXIF-Aufnahmedatum, Kamera, Objektiv, GPS und Ausrichtung werden beim Indizieren von Lap aus der Datei gelesen.
- Das Speichern einer integrierten Bildbearbeitung schreibt das resultierende Bild an den gewählten Speicherort.
- Wenn Sie Dateien **in Lap** umbenennen, verschieben, kopieren oder löschen, aktualisiert Lap gleichzeitig seinen lokalen Katalog. Unterstützte zusammengehörige Dateien wie Apple-Live-Photo-Komponenten, AAE-Seitendateien und aktivierte RAW- + JPEG/HEIC-Paare werden dabei zusammengehalten.

### Was Lap lokal speichert

Die folgenden Informationen sind Bibliotheksdaten von Lap. Sie werden in der lokalen Datenbank oder Bibliothekskonfiguration von Lap gespeichert und nicht in EXIF, IPTC oder XMP-Seitendateien geschrieben:

- Sammlungen, Tags, Kommentare, Favoriten, Bewertungen und Auswahl-Status (Ausgewählt und Abgelehnt)
- Smart-Alben sowie deren Regeln, Gruppierung, Sortierung und Reihenfolge
- KI-Suchdaten, Gesichtsdaten, Thumbnails und weitere Index- bzw. Cache-Daten

Diese Daten reisen nicht mit einer Datei mit, wenn sie außerhalb von Lap kopiert, exportiert oder verschoben wird, und stehen anderen Anwendungen nicht automatisch zur Verfügung.

### Arbeiten mit Dateien außerhalb von Lap

Lap kann Ordner erneut scannen und viele Änderungen im Dateisystem erkennen. Werden Dateien jedoch außerhalb von Lap umbenannt, verschoben, ersetzt oder kopiert, können nur in Lap gespeicherte Organisationsdaten beeinträchtigt werden.

Für die zuverlässigsten Ergebnisse sollten Sie Dateien in Lap umbenennen und verschieben, wenn Sie Sammlungen, Tags, Kommentare, Favoriten, Bewertungen oder Auswahl-Status verwenden. Wenn Sie Dateien auch außerhalb von Lap verwalten, sichern Sie die Datenbank und Konfiguration von Lap zusammen mit Ihren Fotos. Den Datenbankort können Sie in **Einstellungen → Speicher** verwalten und dort auch eine Sicherung erstellen.

Das Löschen der Datenbank oder Konfiguration von Lap entfernt diese lokale Organisation und Indexdaten, löscht jedoch nicht Ihre Originalmediendateien.

## Lap deinstallieren

Lap arbeitet direkt mit Ihren vorhandenen Fotoordnern. Die Deinstallation von Lap oder das Löschen der Datenbank- und Cache-Dateien löscht **nicht** Ihre Originalfotos.

Die normale Deinstallation entfernt die Anwendung. Um Lap vollständig zu entfernen, beenden Sie Lap zuerst, deinstallieren Sie die Anwendung und löschen Sie anschließend die lokale Datenbank, den Thumbnail-Cache und die Konfigurationsdateien mit den Befehlen für Ihre Plattform.

### macOS

Wenn Sie Lap mit Homebrew installiert haben:

```bash
brew uninstall --cask lap
```

Bei einer manuellen Installation beenden Sie Lap und verschieben Sie `Lap.app` aus dem Ordner `Applications` in den Papierkorb.

So entfernen Sie alle Datenbank-, Cache- und Konfigurationsdateien von Lap:

```bash
rm -rf "$HOME/Library/Application Support/com.julyx10.lap" \
       "$HOME/Library/Caches/com.julyx10.lap" \
       "$HOME/Library/WebKit/com.julyx10.lap"
rm -f "$HOME/Library/Preferences/com.julyx10.lap.plist"
```

### Windows

Öffnen Sie **Einstellungen > Apps > Installierte Apps**, suchen Sie **Lap** und wählen Sie **Deinstallieren**.

Öffnen Sie anschließend PowerShell und entfernen Sie alle Datenbank-, Cache- und Konfigurationsdateien von Lap:

```powershell
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$env:LOCALAPPDATA\com.julyx10.lap"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$env:APPDATA\com.julyx10.lap"
```

### Linux

Deinstallieren Sie das Paket auf Debian-basierten Distributionen:

```bash
sudo apt remove lap
```

Entfernen Sie anschließend alle Datenbank-, Cache- und Konfigurationsdateien von Lap:

```bash
rm -rf "$HOME/.local/share/com.julyx10.lap" \
       "$HOME/.cache/com.julyx10.lap" \
       "$HOME/.config/com.julyx10.lap"
```

Wenn Sie in den Lap-Einstellungen ein benutzerdefiniertes Verzeichnis für die Datenbank ausgewählt haben, löschen Sie dieses Verzeichnis separat, nachdem Sie geprüft haben, dass es nur Lap-Datenbankdateien enthält.

## Aus dem Quellcode erstellen

Anforderungen: Node.js 20+, pnpm, Rust stabil.

```bash
# macOS System-Abhängigkeiten
xcode-select --install
brew install nasm pkg-config autoconf automake libtool cmake

# Linux System-Abhängigkeiten
# sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev \
#   patchelf nasm clang pkg-config autoconf automake libtool cmake

# Klonen und Erstellen
git clone --recursive https://github.com/julyx10/lap.git
cd lap
git submodule update --init --recursive
cargo install tauri-cli --version "^2.0.0" --locked
./scripts/download_models.sh            # Windows: .\scripts\download_models.ps1
./scripts/download_ffmpeg_sidecar.sh    # Windows: .\scripts\download_ffmpeg_sidecar.ps1
cd src-vite && pnpm install && cd ..
cargo tauri dev
```

## Unterstützte Formate

Lap unterstützt über 60 Foto-, RAW- und Videoformate.

| Typ | Formate |
| :--- | :--- |
| Bilder | JPG/JPEG/JFIF, PNG, GIF, BMP, TIFF, WebP, HEIC/HEIF/HIF, AVIF, JXL, PSD, EXR, HDR/RGBE, TGA, JPEG 2000 (JP2/J2K/J2C/JPC/JPF/JPX), DDS, DPX, QOI |
| RAW-Fotos | CR2, CR3, CRW, NEF, NRW, ARW, SRF, SR2, RAF, RW2, ORF, PEF, DNG, SRW, RWL, MRW, 3FR, MOS, DCR, KDC, ERF, MEF, RAW, MDC |
| Videos | MP4, MOV, M4V, MKV, AVI, FLV, TS/M2TS, WMV, WebM, 3GP/3G2, F4V, VOB, MPG/MPEG, ASF, DIVX und weitere. Die H.264-Wiedergabe wird auf allen Plattformen unterstützt, mit automatischer Kompatibilitätsverarbeitung, wenn die native Wiedergabe nicht verfügbar ist. HEVC/H.265 und VP9 werden nativ auf macOS unterstützt. |

### Linux Video-Wiedergabe Hinweise

Installieren Sie unter Linux Mint/Ubuntu/Debian diese Pakete für eine bessere Unterstützung der Videowiedergabe:

```bash
sudo apt install gstreamer1.0-libav gstreamer1.0-plugins-good
```

## Architektur

- Kern: Tauri + Rust
- Frontend: Vue + Vite + Tailwind CSS
- Daten: SQLite

### Wichtige Bibliotheken

| Bibliothek | Zweck |
| :-- | :-- |
| [LibRaw](https://github.com/LibRaw/LibRaw) | RAW-Bilddekodierung und Thumbnail-Extraktion |
| [libheif](https://github.com/strukturag/libheif) | HEIC/HEIF/HIF-Bilddekodierung und Vorschaugenerierung |
| [libjpeg-turbo](https://libjpeg-turbo.org/) | Schnelle JPEG-Dekodierung und Thumbnail-Generierung |
| [FFmpeg](https://ffmpeg.org/) | Videoverarbeitung und Thumbnail-Generierung |
| [Video.js](https://videojs.com/) | Plattformübergreifende Benutzeroberfläche für die Videowiedergabe |
| [ONNX Runtime](https://onnxruntime.ai/) | Lokale KI-Modell-Inferenz-Engine |
| [CLIP](https://github.com/openai/CLIP) | Bild-Text-Ähnlichkeitssuche |
| [InsightFace](https://github.com/deepinsight/insightface) | Gesichtserkennung und -identifizierung |
| [Leaflet](https://leafletjs.com/) | Interaktive Karte für Fotos mit GPS-Daten |
| [daisyUI](https://daisyui.com/) | UI-Komponentenbibliothek |

## Lizenz

GPL-3.0-oder-später. Siehe [LICENSE](../LICENSE).
