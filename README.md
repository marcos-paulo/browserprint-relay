# browserprint-relay

Relay TCP com ícone na bandeja do Windows. Ele escuta em `0.0.0.0:19100`
(todas as interfaces de rede) e repassa cada conexão pro Browser Print
de verdade, que continua rodando só em `127.0.0.1:9100`.

- Ícone verde = relay no ar, repassando.
- Ícone vermelho = erro (ex: porta já em uso por outro processo).
- Não mexe no Browser Print. Só fica na frente dele, "escutando pela
  rede" e entregando pro processo local.

Clientes de outras máquinas na rede apontam pro IP dessa máquina na
porta **19100** por padrão (não 9100) — ver "Por que portas diferentes"
abaixo. O menu da bandeja mostra o endereço exato pra usar, e tem opção
de trocar a porta sem precisar mexer em código (ver "Rodar" abaixo).

Confirme antes de instalar: a porta padrão do Browser Print pode variar
conforme a versão/instalação (normalmente 9100). Se for diferente,
troque `BROWSER_PRINT_PORT` em `src/main.rs` (essa aqui não tem opção
no menu — é fixa em tempo de compilação, já que é a porta de outro
programa, não do relay).

## Por que portas diferentes (19100 -> 9100), não a mesma

A primeira versão deste relay tentava escutar em `0.0.0.0:9100` — a
mesma porta do Browser Print. Deu erro "acesso negado" (`WSAEACCES`,
erro do Windows `10013`) ao tentar abrir.

A causa, confirmada com `netstat -ano | findstr :9100`: o próprio
Browser Print já escuta em `0.0.0.0:9100` (todas as interfaces, não só
`127.0.0.1`). Ou seja, não era endereço diferente disputando a mesma
porta — era a mesma tupla endereço+porta exata que o relay tentava usar
de novo. Conflito comum, só que o Windows reportou como "acesso
negado" em vez do esperado "porta já em uso" (`10048`) — o que rendeu
uma investigação mais longa do que precisava até aparecer o
`netstat` confirmando.

(Detalhe que continua valendo, mesmo não tendo sido o caso aqui: no
Windows, se um processo abre uma porta com a flag `SO_EXCLUSIVEADDRUSE`
— comum em serviços bem-comportados —, nenhum outro processo consegue
bindar aquele número de porta, **nem em endereço diferente**. No Linux
isso não existe: endereços diferentes convivem livremente no mesmo
número de porta.)

Também vale lembrar (ver seção seguinte, "Firewall") que só porque o
Browser Print está em `0.0.0.0` não significa que ele já estava
acessível pela rede: testamos com `curl` de outra máquina e ele mesmo
recusou com "Remote access not allowed" — uma checagem própria dele
sobre a origem da conexão, sem relação com bind de endereço ou
firewall. É exatamente esse bloqueio que o relay contorna: a conexão
que ele abre com o Browser Print parte da própria máquina (localhost
pro Browser Print), então nunca é vista como remota.

De qualquer forma, o relay usa uma porta própria (19100) pra escutar, e
só usa a 9100 como destino de pra onde repassar — evita depender de
qual das duas explicações se aplica em cada máquina.

## Build nativo (numa máquina Windows)

1. Instalar Rust (rustup.rs).
2. `cargo build --release`
3. Binário fica em `target\release\browserprint-relay.exe`.

## Cross-compile a partir do Linux (Fedora)

```
sudo dnf install mingw64-gcc mingw64-gcc-c++
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

Binário fica em `target/x86_64-pc-windows-gnu/release/browserprint-relay.exe`.

## Release (branch órfã com o .exe)

`scripts/release.sh` builda o `.exe` (cross-compile, igual acima) e
publica ele sozinho numa branch órfã — sem histórico acumulado, cada
execução sobrescreve a anterior.

```
scripts/release.sh [nome-da-branch]   # padrão: "release"
```

Regras:

- Só roda se não houver nada pendente pra commitar (`git status` tem
  que estar limpo) — a release sempre corresponde a um commit real,
  nunca a um work-in-progress.
- O nome do `.exe` publicado sempre leva o hash curto do último commit:
  `browserprint-relay-rev.<hash>.exe`. Assim dá pra saber de qual
  commit cada release veio.
- Confere sozinho se `rustup target add x86_64-pc-windows-gnu` e o
  `mingw64-gcc`/`mingw64-gcc-c++` (ver seção de cross-compile acima)
  estão instalados — se faltar algum, para com uma mensagem dizendo o
  que instalar, não instala nada sozinho.
- Se o repo tiver remote `origin` configurado — como este, que já
  aponta pro GitHub — dá `push --force` da branch pro remote. Sem
  remote, a branch fica só local e o script mostra o comando pra pegar
  o binário dela mesmo assim.

## Firewall do Windows

Bindar em `0.0.0.0` não basta — o Firewall do Windows ainda decide se
aceita a conexão de fora. No primeiro `.exe` rodando, o Windows deve
perguntar se libera o app na rede (marcar rede privada, não pública).
Se não perguntar, liberar manualmente:

```
netsh advfirewall firewall add rule name="Browser Print Relay" dir=in action=allow protocol=TCP localport=19100
```

(precisa rodar como administrador — não incluí isso no app porque
mexer no firewall merece confirmação explícita, não automação
silenciosa.)

## Rodar

Só executar o `.exe`. Ele sobe minimizado na bandeja — sem janela. Menu
de contexto no ícone mostra:

- **Status** — se está rodando e em qual porta, ou o erro (se houver).
- **Endereço** — o IP:porta que o app que vai imprimir precisa
  configurar (ex.: `192.168.0.42:19100`). Detectado sozinho (mesmo IP
  que aparece no `ipconfig`, na interface da rota padrão).
- **Copiar endereço** — copia esse IP:porta pra área de transferência,
  pra colar direto na configuração de quem for usar.
- **Trocar porta…** — abre uma caixa de diálogo pra digitar uma porta
  nova. Aplica na hora (derruba o listener antigo, sobe um novo — não
  precisa reiniciar o app) e já salva pra próxima vez que abrir.
- **Sair**.

Clientes de outras máquinas na rede apontam pro endereço mostrado no
menu (IP dessa máquina + porta do relay — 19100 por padrão).

## Onde fica salva a porta escolhida

No Registro do Windows, `HKEY_CURRENT_USER\Software\browserprint-relay`
(valor `RelayPort`) — sem precisar de arquivo de configuração nem banco
de dados. É por usuário: se outra conta do Windows logar na mesma
máquina, começa com a porta padrão (19100) até trocar também.
