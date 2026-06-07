<div align="center">

# ⏳ ShelfLife

### _Keep your files tidy_.

**ShelfLife** is an elegant, open-source, and lightweight desktop utility that treats specified folders (such as your Downloads and Desktop directories) as temporary staging areas. Files are assigned a lifespan; unpinned files automatically decay into the system trash after a configured duration to prevent digital hoarding.

[![Rust](https://img.shields.io/badge/Rust-ea4a31?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Svelte 5](https://img.shields.io/badge/Svelte_5-ff3e00?style=for-the-badge&logo=svelte&logoColor=white)](https://svelte.dev/)
[![Tauri v2](https://img.shields.io/badge/Tauri_v2-24c8db?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![Windows](https://img.shields.io/badge/Windows_Optimized-0078d4?style=for-the-badge&logo=windows&logoColor=white)](#)
[![License: GPLv3](https://img.shields.io/badge/License-GPLv3-blue.svg?style=for-the-badge)](https://www.gnu.org/licenses/gpl-3.0.html)

</div>

---

## 🌟 Product Philosophy & Core Promise

ShelfLife helps you reduce digital clutter without losing trust in your filesystem. It is intentionally cautious—**never** behaving like an aggressive cleaner. Instead, it behaves like a file triage assistant: it observes, classifies, explains, asks, acts, and logs.

> [!IMPORTANT]  
> **Safety First:** The application never surprises you with irreversible changes. All file-modifying operations are explicit, fully logged in an audit ledger, and reversible (via native system Recycle Bin integration) where technically possible.

---

## ✨ Features

- 🕒 **Ambient Decay States:** Files flow naturally through four status tiers: **Fresh** (active/recent), **Stale** (inactive), **Decaying** (retention window closing soon), and **Pinned** (safely protected indefinitely).
- ⚡ **Event-Driven Directory Watcher:** Powered by native OS filesystem hooks (Rust `notify` crate) instead of heavy folder polling. Zero idle CPU overhead.
- 🛠️ **User-Controlled Rule Engine:** Custom rules are created in `PreviewOnly` mode by default, allowing you to observe what files _would_ match before promoting them to `AskFirst` or `Automatic` actions.
- 🗃️ **Safe Trashing:** Integrates directly with Windows `IFileOperation` (via the `trash` crate) to move items safely to the Recycle Bin rather than deleting them permanently.
- 🕵️ **Origin Tracking:** Automatically inspects Windows Zone.Identifier Alternate Data Streams (ADS) to display which domains or referrer URLs a file was downloaded from, aiding triage.
- 🗒️ **Audit Ledger & Robust Undo:** Keeps a complete log of all actions for 30+ days. If you accidentally trash or move a file, restore it with a single click.
- 🪶 **Minimal Footprint:** Compiled with Rust and Tauri. Runs smoothly on less than 30MB of RAM.

---

## 📄 License

Distributed under the GNU General Public License v3.0 (GPLv3). See [LICENSE](file:///i:/Project/ShelfLife/LICENSE) for details.
