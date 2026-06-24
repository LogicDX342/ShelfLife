<div align="center">

# ShelfLife

### _Keep your files tidy_.

[![GitHub release](https://img.shields.io/github/v/release/LogicDX342/ShelfLife?style=flat-square&color=24c8d8)](https://github.com/LogicDX342/ShelfLife/releases)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows-0078d7.svg?style=flat-square&logo=windows)](https://microsoft.com/windows)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg?style=flat-square)](LICENSE)

[![Tauri](https://img.shields.io/badge/Tauri-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-black?style=flat-square&logo=rust&logoColor=E05D44)](https://www.rust-lang.org/)
[![Svelte](https://img.shields.io/badge/Svelte-FF3E00?style=flat-square&logo=svelte&logoColor=white)](https://svelte.dev/)

**ShelfLife** is an elegant, open-source, and lightweight desktop utility that treats specified folders (such as your Downloads and Desktop directories) as temporary staging areas. Files are assigned a lifespan; unpinned files automatically decay into the system trash after a configured duration to prevent digital hoarding.

<br/>

**English** | [简体中文](README_zh.md)

</div>

---

ShelfLife is an open-source desktop utility that helps you maintain order in your Downloads and Desktop folders. It treats these directories as temporary storage areas, automatically moving older files to the trash after a time you specify. This approach prevents digital clutter while giving you full control over your files.

## Core Principles

ShelfLife is designed with caution and transparency in mind. It never makes unexpected or irreversible changes to your files. All actions are clearly logged, and files are moved to the system Recycle Bin, allowing for easy recovery if needed. The application acts as a helpful assistant, observing, categorizing, and acting on your files only as you direct.

## Key Features

- **File Status Tracking:** Files are classified into clear states:
  - **Fresh:** Recently added or accessed.
  - **Stale:** Inactive for a set period.
  - **Decaying:** Approaching the end of its lifespan.
  - **Pinned:** Protected and never automatically moved.
- **Efficient Monitoring:** Uses your operating system's built-in file events to watch folders, consuming negligible system resources when idle.
- **Customizable Rules:** Create rules to manage files. New rules start in a safe "preview" mode, letting you see which files would be affected before you activate them.
- **Safe File Removal:** Integrates with your system's Recycle Bin to ensure files can be restored if necessary.
- **Download Source Info:** Automatically shows you where a downloaded file originated from, helping you decide what to keep.
- **Complete Activity Log:** Maintains a detailed history of all actions for over 30 days, allowing you to undo any automatic move with a single click.
- **Lightweight:** Built with Tauri, it runs smoothly using very little of your computer's memory.

## Interface Preview

<table border="0" cellpadding="0" cellspacing="0" width="100%">
  <tr>
    <td align="center" valign="top" width="50%">
      <strong>Dashboard</strong>
      <br />
      <br />
      <img src="docs/images/dashboard.png" alt="File Status Dashboard" width="95%" />
    </td>
    <td align="center" valign="top" width="50%">
      <strong>Review Queue</strong>
      <br />
      <br />
      <img src="docs/images/review.png" alt="Review Queue" width="95%" />
    </td>
  </tr>
  <tr>
    <td align="center" valign="top" width="50%">
      <br />
      <br />
      <strong>Rule Editor</strong>
      <br />
      <br />
      <img src="docs/images/rules.png" alt="Rule Configuration" width="95%" />
    </td>
    <td align="center" valign="top" width="50%">
      <br />
      <br />
      <strong>Activity Log</strong>
      <br />
      <br />
      <img src="docs/images/audit.png" alt="Activity Log" width="95%" />
    </td>
  </tr>
</table>

## License

ShelfLife is distributed under the GNU General Public License v3.0 (GPLv3). See the [LICENSE](./LICENSE) file for more information.
