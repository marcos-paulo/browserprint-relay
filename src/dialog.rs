use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

// CREATE_NO_WINDOW -- evita o flash de uma janela de console atrás da caixa
// de diálogo, já que o app roda sem console (windows_subsystem = "windows").
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Pede um texto ao usuário através da caixa de input do VB (via PowerShell),
/// em vez de trazer um toolkit de GUI inteiro só pra um campo de texto.
/// Comum em utilitários pequenos de bandeja no Windows.
///
/// Retorna `None` se o usuário cancelar, fechar a janela, ou se algo falhar
/// ao rodar o PowerShell.
pub fn pedir_texto(titulo: &str, mensagem: &str, valor_padrao: &str) -> Option<String> {
    let script = format!(
        "Add-Type -AssemblyName Microsoft.VisualBasic; \
         [Microsoft.VisualBasic.Interaction]::InputBox('{}', '{}', '{}')",
        escapar_aspas_simples(mensagem),
        escapar_aspas_simples(titulo),
        escapar_aspas_simples(valor_padrao),
    );

    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let saida = cmd.output().ok()?;
    if !saida.status.success() {
        return None;
    }

    let texto = String::from_utf8_lossy(&saida.stdout).trim().to_string();
    if texto.is_empty() {
        None
    } else {
        Some(texto)
    }
}

// PowerShell usa aspas simples dobradas ('') pra escapar aspas simples dentro
// de uma string entre aspas simples.
fn escapar_aspas_simples(texto: &str) -> String {
    texto.replace('\'', "''")
}
