#!/usr/bin/env bash
# Faz uma release rastreável ao commit atual: builda o binário Windows (cross-compile via
# target x86_64-pc-windows-gnu) e publica ele sozinho numa branch órfã (sobrescrita a cada
# execução, sem acumular histórico), com o hash curto do commit no nome do arquivo -- pra
# saber exatamente de qual commit cada .exe publicado veio. Só roda se não houver NADA
# pendente pra commitar -- garante que o binário publicado corresponde exatamente a um
# estado já commitado, nunca a um work-in-progress.
#
# Baseado em scripts/release.sh + scripts/publicar-pacote-vdi.sh do projeto
# iib-comunicacao, simplificado pra um binário Rust único (sem gerenciador de pacote JS
# envolvido, então sem versão de package.json pra trocar/restaurar).
#
# Uso: scripts/release.sh [nome-da-branch]
set -euo pipefail

RAIZ_DO_PROJETO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RAIZ_DO_PROJETO"

NOME_BINARIO="browserprint-relay"
TARGET="x86_64-pc-windows-gnu"
BRANCH_ORFA="${1:-release}"
REMOTO="origin"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "[release] Há alterações não commitadas -- commit (ou stash) tudo antes de fazer uma release:" >&2
  git status --short >&2
  exit 1
fi

if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "[release] Target $TARGET não instalado. Rode: rustup target add $TARGET" >&2
  exit 1
fi

if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
  echo "[release] Falta o linker mingw-w64 (x86_64-w64-mingw32-gcc)." >&2
  echo "[release] No Fedora: sudo dnf install mingw64-gcc mingw64-gcc-c++" >&2
  exit 1
fi

HASH_CURTO="$(git rev-parse --short HEAD)"
BRANCH_ATUAL="$(git rev-parse --abbrev-ref HEAD)"

echo "[release] Branch: $BRANCH_ATUAL -- commit: $HASH_CURTO"
echo "[release] Buildando $NOME_BINARIO em release ($TARGET)..."
cargo build --release --target "$TARGET"

BIN_ORIGEM="target/$TARGET/release/${NOME_BINARIO}.exe"
if [[ ! -f "$BIN_ORIGEM" ]]; then
  echo "[release] Binário não encontrado em $BIN_ORIGEM" >&2
  exit 1
fi

# Sufixo sempre com o último commit -- assim cada execução gera um arquivo com nome
# diferente (rastreável ao commit de origem), e não sobrescreve silenciosamente uma
# release anterior de outro commit.
NOME_RELEASE="${NOME_BINARIO}-rev.${HASH_CURTO}.exe"
WORKTREE_TMP="$(mktemp -d)"

limpar() {
  git worktree remove --force "$WORKTREE_TMP" >/dev/null 2>&1 || true
  rm -rf "$WORKTREE_TMP"
}
trap limpar EXIT

echo "[release] Criando branch órfã '$BRANCH_ORFA' num worktree temporário..."
# --orphan exige uma branch "não nascida" -- não dá pra combinar com -B pra reaproveitar
# uma branch que já tem commit de uma execução anterior (erro: "already exists"). Por
# isso apagamos a branch local antes, se existir, e recriamos do zero toda vez.
git branch -D "$BRANCH_ORFA" >/dev/null 2>&1 || true
git worktree add --orphan -b "$BRANCH_ORFA" "$WORKTREE_TMP" --quiet

cp "$BIN_ORIGEM" "$WORKTREE_TMP/$NOME_RELEASE"

(
  cd "$WORKTREE_TMP"
  git add "$NOME_RELEASE"
  git commit -m "Release: $NOME_RELEASE" --quiet
)

echo
if git remote get-url "$REMOTO" >/dev/null 2>&1; then
  echo "[release] Publicando em '$REMOTO/$BRANCH_ORFA' (force-push)..."
  (cd "$WORKTREE_TMP" && git push "$REMOTO" "$BRANCH_ORFA" --force)
  echo "[release] Pronto. Baixar com:"
  echo "  git clone --branch $BRANCH_ORFA --single-branch <url-do-repo> ${NOME_BINARIO}-release"
else
  echo "[release] Sem remote '$REMOTO' configurado -- a branch '$BRANCH_ORFA' ficou só local."
  echo "[release] Pra pegar o binário sem configurar remote:"
  echo "  git --work-tree=<destino> checkout $BRANCH_ORFA -- $NOME_RELEASE"
fi
