# Project structure

- apps/
  - designcraft/, native application
  - designcraft-cli/, headless commands and automation
  - designcraft-web/, WASM application
- assets/
  - app-icon/
  - fonts/
  - hyphenation/
  - icc/
- contributors/, contributor credit inputs
- crates/
  - color/
  - compose/
  - doc/
  - engine/
  - epub/
  - fonts/
  - format/
  - geom/
  - idml/
  - images/
  - mcp/
  - pdf/
  - pptx/
  - render/
  - textimport/
  - tools/
  - ui-egui/
- docs/
  - brand/
  - context/, detailed agent context and this structure
  - images/
  - user/, user documentation
  - index.md, symbol map index
  - interactive-panels.md, panel symbol map
- licenses/pdf-backend/
- nix/, Nix packaging
- packaging/
  - freebsd/
  - linux/
  - macos/
  - web/
  - windows/
- xtask/src/, repository checks
- AGENTS.md, repository contribution rules
- ASSETS.md, asset attribution
- Cargo.toml and Cargo.lock, workspace and dependencies
- ROADMAP.md, repository progress log
.
├── .agents
├── .aws
├── .cargo
│   └── config.toml
├── .coderabbit.yaml
├── .codex
├── .config
│   └── nextest.toml
├── .github
│   ├── dependabot.yml
│   └── workflows
├── .gitignore
├── AGENTS.md
├── ASSETS.md
├── CLAUDE.md
├── Cargo.lock
├── Cargo.toml
├── LICENSE-APACHE
├── LICENSE-MIT
├── NOTICE
├── README.md
├── ROADMAP.md
├── apps
│   ├── designcraft
│   ├── designcraft-cli
│   └── designcraft-web
├── assets
│   ├── app-icon
│   ├── fonts
│   ├── hyphenation
│   └── icc
├── clippy.toml
├── contributors
│   ├── contributors.json
│   └── people.toml
├── crates
│   ├── color
│   ├── compose
│   ├── doc
│   ├── engine
│   ├── epub
│   ├── fonts
│   ├── format
│   ├── geom
│   ├── idml
│   ├── images
│   ├── mcp
│   ├── pdf
│   ├── pptx
│   ├── render
│   ├── textimport
│   ├── tools
│   └── ui-egui
├── docs
│   ├── agents.md
│   ├── arabic-typography.md
│   ├── architecture.md
│   ├── brand
│   ├── cjk-typography.md
│   ├── cjk.md
│   ├── context
│   ├── contributors.md
│   ├── control-protocol.md
│   ├── egui-037-migration-plan.md
│   ├── file-format-parity.md
│   ├── find-change-symbols.md
│   ├── gaps.md
│   ├── graphic-layer-options.md
│   ├── hardware-parity.md
│   ├── images
│   ├── index.md
│   ├── linked-artwork.md
│   ├── localization-parity.md
│   ├── mcp.md
│   ├── mojikumi.md
│   ├── multi-image-placement.md
│   ├── parity-checklist.md
│   ├── pdf-backend.md
│   ├── pdf-layer-visibility.md
│   ├── roadmap-fragments
│   ├── roadmap.md
│   ├── target-app-parity.md
│   ├── testing-variable-fonts.md
│   ├── typography-parity.md
│   ├── ui-parity.md
│   └── user
├── flake.lock
├── flake.nix
├── licenses
│   └── pdf-backend
├── nix
│   └── package.nix
├── packaging
│   ├── .shellcheckrc
│   ├── env.sh
│   ├── freebsd
│   ├── icons.sh
│   ├── linux
│   ├── macos
│   ├── web
│   └── windows
├── rustfmt.toml
├── sonar-project.properties
└── xtask
    ├── Cargo.toml
    └── src
.
|-- apps
|   |-- designcraft
|   |-- designcraft-cli
|   `-- designcraft-web
|-- assets
|   |-- app-icon
|   |-- fonts
|   |-- hyphenation
|   `-- icc
|-- contributors
|-- crates
|   |-- color
|   |-- compose
|   |-- doc
|   |-- engine
|   |-- epub
|   |-- fonts
|   |-- format
|   |-- geom
|   |-- idml
|   |-- images
|   |-- mcp
|   |-- pdf
|   |-- pptx
|   |-- render
|   |-- textimport
|   |-- tools
|   `-- ui-egui
|-- docs
|   |-- brand
|   |-- context
|   |-- images
|   `-- user
|-- licenses
|   `-- pdf-backend
|-- nix
|-- packaging
|   |-- freebsd
|   |-- linux
|   |-- macos
|   |-- web
|   `-- windows
`-- xtask
    `-- src
