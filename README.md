# Warframe Market Syndicate Automator

<div align="center">

[![Tauri v2](https://img.shields.io/badge/Tauri-v2-FFC107?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-Backend-000000?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![React & TS](https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=black)](https://react.dev/)
</div>

> [!WARNING]
> This project is a personal learning experience built with extensive use of AI assistance. It is completely independent and not affiliated with warframe.market. Automating platform interactions may violate their Terms of Service. Use at your own risk.

A lightweight, ultra-performance native desktop application engineered for **Warframe** players to seamlessly track and manage syndicate standing, calculate listable offerings, automate undercutting pricing, and publish live sell listings directly to the **warframe.market** REST APIs.

---

## ✨ Features & Capabilities

- ** Secure In-App Authentication**: Integrates an embedded secure login flow using Tauri's native `cookies` API. It automatically extracts the `JWT` cookie upon signing in at `warframe.market`.
- ** Interactive Standing Ledger**: Synchronize and adjust faction standing in real-time with responsive sliders. Max standing caps are dynamically recalculated based on your represented faction rank dropdown.
- ** Smart Offering Publisher**: Automatically queries the lowest active competitor prices from `warframe.market` for your represented faction's mods, undercuts the competition by `1 Platinum`, and registers/updates live sell listings. Deletes listings instantly if standing falls to zero.
- ** Cross-Faction Conflict Resolver**: When logging a sale for an offering represented by multiple active syndicates, a frosted-glass overlay modal prompts you to select the attributing syndicate. Standing values are automatically updated on-disk and active market listings cascade-synced!
- ** Local Persistence**: Standings, rank levels, configuration options, and encrypted session keys are preserved locally on-disk.
- ** Integrated Auto-Updater**: Periodically checks for updates and renders an elegant, non-intrusive banner to download, install, and relaunch the application in one click.

---

## 🛠️ Tech Stack & Architecture

### Frontend

- **Framework**: Vite + React 19 (TypeScript)
- **Styling**: Vanilla CSS custom glassmorphism (frosted backgrounds, backdrop-filters, custom-scoped variables, interactive CSS transitions, and responsive grid layouts)

### Backend

- **Core**: Tauri v2 in Rust
- **APIs & State**: Safe thread-local disk persistence, native cookie store monitoring, Tokio background tasks, and asynchronous HTTP client routines powered by `reqwest`.

---

## 🚀 Getting Started

### 1. Prerequisites

Ensure you have the following installed on your machine:

- [Node.js](https://nodejs.org/) (v18+)
- [Rust & Cargo Compiler](https://www.rust-lang.org/tools/install) (v1.75+)

### 2. Developer Commands

Open your terminal in the root project directory and execute:

```bash
# Install dependencies
bun install

# Run the Tauri application in hot-reloading development mode
bun run tauri dev

# Clean and bundle the frontend assets only (useful for visual adjustments)
bun run dev
```

---

## 📖 Operational Guide

1.  **Sync standing:** Adjust the sliders for each represented faction to match your in-game standing values. Standing caps are automatically recalculated if you adjust your faction rank dropdown.
2.  **Publish offerings:** Click **"Publish Offerings"** for a faction. The Rust engine queries the lowest active prices for that faction's mods, undercuts them by `1 Platinum`, and posts/updates live sell orders on your behalf.
3.  **Log sales:** Select a sold mod from the dropdown list, enter the quantity, and click **"Log Completed Sale"**.
4.  **Resolve conflicts (Attribution Modal):** If the sold mod belongs to multiple active factions you represent, an elegant frosted-glass modal overlay will prompt you to select which syndicate's pool to deduct the standing from. The backend will automatically adjust standings and cascade update all your active market listings!

---

## 🔮 Planned Roadmap & TODOs

- [ ] **Real-Time Price Monitoring & Re-undercutting:** A background worker in the Rust backend to monitor competitor price shifts and automatically prompt adjustments or dynamically underbid to keep listings active.
- [ ] **Customizable Pricing Strategy:** Allow users to set their own undercutting margins (e.g., matching lowest price instead of undercutting, or setting custom Platinum values) and establish a "price floor" to avoid selling rare mods too cheap.
- [ ] **Advanced Profit & Sales Analytics:** A dashboard view showing historical sales logs, total Platinum earned, and standing-to-platinum efficiency metrics per syndicate.
- [ ] **Multi-Profile Support:** Capability to manage and switch between multiple `warframe.market` user accounts or game platform profiles (PC, PlayStation, Xbox, Switch).
- [ ] **Intelligent Offering Planner:** Recommend the most optimal mods/offerings to sell based on current daily market demand and standing conversion efficiency.
- [x] **Automatic Session Token Refresh:** Detection of expired JWT tokens and seamless automated re-authentication via the in-app login window.
