# Perene v2 — atalhos de build.
#
# Só embrulha os comandos que já existem (cargo + npm run tauri); nada aqui é
# requisito pra compilar o projeto. O que justifica o arquivo são as diferenças
# chatas entre plataformas — principalmente o bundling no Linux (ver `bundle`).

SHELL := /bin/bash
.DEFAULT_GOAL := help

DESKTOP := apps/desktop
UNAME_S := $(shell uname -s)

# Onde o `install` joga as coisas no Linux (user-local, sem sudo).
PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin
APPDIR := $(PREFIX)/share/applications
ICONDIR := $(PREFIX)/share/icons/hicolor

# Formatos de instalador. No Linux o default é deb,rpm de propósito: o AppImage
# quebra em distros que não têm mais /usr/lib/gdk-pixbuf-2.0/2.10.0 (Arch e
# derivados), porque o linuxdeploy-plugin-gtk copia esse diretório às cegas.
# Em Debian/Ubuntu dá pra pedir tudo: `make bundle BUNDLES=all`.
ifeq ($(UNAME_S),Linux)
  BUNDLES ?= deb,rpm
  # linuxdeploy carrega um `strip` antigo que engasga com a seção .relr.dyn das
  # libs de distros novas. Sem isso, `BUNDLES=all` morre antes do AppImage.
  export NO_STRIP := true
else
  BUNDLES ?= all
endif

.PHONY: help
help: ## Lista os targets
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "  Plataforma: $(UNAME_S) · bundles: $(BUNDLES) · prefix: $(PREFIX)"

.PHONY: setup
setup: ## Instala as dependências do front (npm ci)
	cd $(DESKTOP) && npm ci

$(DESKTOP)/node_modules:
	@$(MAKE) setup

.PHONY: dev
dev: $(DESKTOP)/node_modules ## Roda o app com hot reload
	cd $(DESKTOP) && npm run tauri dev

.PHONY: build
build: ## Compila os crates Rust (debug)
	cargo build --workspace

.PHONY: test
test: ## Roda os testes Rust
	cargo test --workspace

.PHONY: check
check: $(DESKTOP)/node_modules ## Typecheck do front (svelte-check)
	cd $(DESKTOP) && npm run check

.PHONY: front
front: $(DESKTOP)/node_modules ## Só o bundle do front (vite)
	cd $(DESKTOP) && npm run build

.PHONY: bundle
bundle: $(DESKTOP)/node_modules ## Gera os instaladores (release)
	cd $(DESKTOP) && npm run tauri build -- --bundles $(BUNDLES)

.PHONY: release
release: ## Compila o binário release, sem empacotar
	cargo build --workspace --release

ifeq ($(UNAME_S),Linux)
# Instalação user-local a partir do binário já compilado. Serve pra distro que
# não come .deb/.rpm (Arch, Manjaro, NixOS…) sem passar por cima do gerenciador
# de pacotes do sistema.
.PHONY: install
install: ## [Linux] Instala em ~/.local a partir do binário release
	@test -x target/release/perene-desktop \
	  || { echo "Rode 'make release' ou 'make bundle' antes."; exit 1; }
	install -Dm755 target/release/perene-desktop $(BINDIR)/perene-desktop
	install -Dm644 $(DESKTOP)/src-tauri/icons/32x32.png \
	  $(ICONDIR)/32x32/apps/perene-desktop.png
	install -Dm644 $(DESKTOP)/src-tauri/icons/64x64.png \
	  $(ICONDIR)/64x64/apps/perene-desktop.png
	install -Dm644 $(DESKTOP)/src-tauri/icons/128x128.png \
	  $(ICONDIR)/128x128/apps/perene-desktop.png
	install -Dm644 $(DESKTOP)/src-tauri/icons/128x128@2x.png \
	  $(ICONDIR)/256x256/apps/perene-desktop.png
	@install -d $(APPDIR)
	@printf '%s\n' \
	  '[Desktop Entry]' 'Type=Application' 'Name=Perene' \
	  'Comment=Terminal manager for AI coding CLIs' \
	  'Exec=$(BINDIR)/perene-desktop' 'Icon=perene-desktop' \
	  'Categories=Development;' 'Terminal=false' \
	  'StartupWMClass=perene-desktop' > $(APPDIR)/Perene.desktop
	@chmod 644 $(APPDIR)/Perene.desktop
	-@update-desktop-database $(APPDIR) 2>/dev/null
	-@gtk-update-icon-cache -f -t $(ICONDIR) 2>/dev/null
	@echo "Instalado em $(BINDIR)/perene-desktop"
	@echo "Ao reinstalar com o app aberto, mate SÓ a UI — ver 'make kill-ui'."

.PHONY: uninstall
uninstall: ## [Linux] Remove o que o install colocou em ~/.local
	rm -f $(BINDIR)/perene-desktop $(APPDIR)/Perene.desktop
	rm -f $(ICONDIR)/*/apps/perene-desktop.png
	-@update-desktop-database $(APPDIR) 2>/dev/null
	@echo "Removido. O estado em ~/.perene2 continua lá."

.PHONY: kill-ui
kill-ui: ## [Linux] Mata só a UI, preservando o daemon (e as sessões)
	@# -x casa o NOME do processo, não a linha de comando: um `grep perene-desktop`
	@# perdido não entra na lista. O -v '--daemon' é o que salva as sessões vivas.
	@pids=$$(pgrep -a -x perene-desktop | grep -v -- '--daemon' | awk '{print $$1}'); \
	if [ -z "$$pids" ]; then \
	  echo "Nenhuma UI rodando."; \
	else \
	  kill $$pids && echo "UI encerrada ($$pids); daemon segue vivo."; \
	fi
endif

.PHONY: clean
clean: ## Remove artefatos de build
	cargo clean
	rm -rf $(DESKTOP)/dist
