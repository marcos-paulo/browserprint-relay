// Guarda a porta escolhida pelo usuário sem precisar de arquivo de config nem
// banco de dados: usa o Registro do Windows (HKEY_CURRENT_USER), que já existe
// pra isso -- por usuário, sobrevive a reinstalação/atualização do .exe, e não
// deixa arquivo solto pra gerenciar.
//
// Um valor por protocolo (HTTP e HTTPS têm portas independentes no Browser
// Print, então o relay também precisa de duas portas próprias -- ver
// main.rs).

pub const CHAVE_REGISTRO: &str = r"Software\browserprint-relay";
pub const VALOR_PORTA_HTTP: &str = "RelayPortHttp";
pub const VALOR_PORTA_HTTPS: &str = "RelayPortHttps";

#[cfg(windows)]
mod imp {
    use super::CHAVE_REGISTRO;
    use winreg::enums::*;
    use winreg::RegKey;

    pub fn carregar_porta(nome_valor: &str, padrao: u16) -> u16 {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey(CHAVE_REGISTRO)
            .and_then(|chave| chave.get_value::<u32, _>(nome_valor))
            .ok()
            .and_then(|valor| u16::try_from(valor).ok())
            .unwrap_or(padrao)
    }

    pub fn salvar_porta(nome_valor: &str, porta: u16) -> anyhow::Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (chave, _) = hkcu.create_subkey(CHAVE_REGISTRO)?;
        chave.set_value(nome_valor, &(porta as u32))?;
        Ok(())
    }
}

// Fora do Windows não existe Registro -- isso só entra em ação quando alguém
// (ex.: `cargo check` local, fora do target de verdade) compila pra outro SO.
// A porta simplesmente não persiste entre execuções nesse caso.
#[cfg(not(windows))]
mod imp {
    pub fn carregar_porta(_nome_valor: &str, padrao: u16) -> u16 {
        padrao
    }

    pub fn salvar_porta(_nome_valor: &str, _porta: u16) -> anyhow::Result<()> {
        Ok(())
    }
}

pub use imp::{carregar_porta, salvar_porta};
