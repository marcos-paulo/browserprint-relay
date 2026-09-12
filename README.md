# browserprint-relay

Relay TCP com ícone na bandeja do Windows. Ele escuta em `0.0.0.0:19100`
(todas as interfaces de rede) e repassa cada conexão pro Browser Print
de verdade, que continua rodando só em `127.0.0.1:9100`.

- Ícone verde = relay no ar, repassando.
- Ícone vermelho = erro (ex: porta já em uso por outro processo).
- Não mexe no Browser Print. Só fica na frente dele, "escutando pela
  rede" e entregando pro processo local.

Clientes de outras máquinas na rede apontam pro IP dessa máquina na
porta **19100** (não 9100) — ver "Por que portas diferentes" abaixo.

Confirme antes de instalar: a porta padrão do Browser Print pode variar
conforme a versão/instalação (normalmente 9100). Se for diferente,
troque `BROWSER_PRINT_PORT` em `src/main.rs`. Pra trocar a porta que o
relay expõe pra rede, troque `RELAY_PORT`.

## Por que portas diferentes (19100 -> 9100), não a mesma

A primeira versão deste relay escutava em `0.0.0.0:9100`, a mesma
porta do Browser Print — em teoria endereços diferentes (`0.0.0.0` vs
`127.0.0.1`) convivem no mesmo número de porta sem conflito. **Isso
vale no Linux, mas não necessariamente no Windows.**

Se o processo que já tem a porta aberta (o Browser Print, nesse caso)
usou a flag `SO_EXCLUSIVEADDRUSE` no bind dele — comum em serviços
Windows bem-comportados, pra evitar que outro processo sequestre a
porta — **nenhum outro processo consegue bindar aquele número de
porta, nem em endereço diferente**. O bind falha com "acesso negado"
(`WSAEACCES`, erro do Windows `10013`), mesmo sem nenhum conflito de
verdade — diferente do erro "porta já em uso" (`10048`) que seria de
esperar.

Foi exatamente esse erro 10013 que apareceu na prática. Por isso o
relay usa uma porta própria (19100) pra escutar, e só usa a 9100 como
destino de pra onde repassar — evita a ambiguidade de vez, em vez de
depender de um comportamento que muda entre sistemas operacionais.

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
- Se o repo tiver remote `origin` configurado, dá `push --force` da
  branch pro remote. Sem remote (caso de hoje), a branch fica só local
  e o script mostra o comando pra pegar o binário dela mesmo assim.

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
de contexto no ícone mostra o status e tem "Sair".

Clientes de outras máquinas na rede então apontam pro IP dessa máquina
na porta 19100.
