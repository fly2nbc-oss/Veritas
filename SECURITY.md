# Security

## Supported versions

Security fixes are made for the latest release only.

| Version | Supported |
|---|---|
| The latest release | yes |
| Older releases | no – please update first |

## Reporting a vulnerability

Please do **not** open a public issue for a security problem. Use GitHub's [private vulnerability reporting](https://github.com/fly2nbc-oss/Veritas/security/advisories/new) or send an e-mail to fly2nbc@gmail.com.

Say which Veritas version you use (F1 ▸ “Über Veritas”, i.e. About Veritas), which operating system, and how to trigger the problem. Please don't attach private files. You get a first answer within 14 days.

## What Veritas does

- **Network:** none. Veritas sends nothing and downloads nothing. Only when you click a link in *About Veritas* does your browser open GitHub.
- **Reading:** every file in the chosen folder, buffered and read-only. Symbolic links are not followed, special files are skipped. Paths from a `.md5` file that contain `..` or are absolute are discarded.
- **Writing:** in the chosen folder only `{folder name}.md5` and the history `{folder name}.json`, atomically through a `.partial` file; an abort writes nothing. In the data folder (Windows `%APPDATA%\Veritas\data`, Linux `~/.local/share/veritas`): the settings (`app.ron`), the run log `veritas.log` (start and end of every run with the folder path, warnings from the window and the graphics) and, after a crash, `crash.log` (both in English). Both logs can contain paths and file names. Each log starts over at 1 MiB.
- **Deleting:** only on Del or from the context menu, always after a confirmation, and only into the trash (Recycle Bin). On Windows not on network and removable drives. Veritas never deletes the `.md5` file or the history.
- **Opening:** a double-click hands the file to the program the system provides for it. Veritas itself executes nothing.
