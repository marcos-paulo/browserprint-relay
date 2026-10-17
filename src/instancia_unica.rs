// Impede duas instâncias do relay ao mesmo tempo: as duas disputariam as
// mesmas portas, e a segunda ficaria só com um ícone de erro na bandeja.
//
// Usa um mutex nomeado do Windows. O nome é visível pra todos os processos da
// mesma sessão de login (prefixo `Local\`). O Windows libera o mutex sozinho
// quando o processo termina, mesmo se ele cair -- não sobra trava presa.

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    const NOME_MUTEX: &str = r"Local\browserprint-relay";

    /// Devolve `true` se esta é a única instância; `false` se já há outra.
    pub fn adquirir() -> bool {
        let nome: Vec<u16> = NOME_MUTEX.encode_utf16().chain(std::iter::once(0)).collect();

        // O handle não é fechado de propósito: ele precisa durar até o fim do
        // processo, que é quando o Windows libera o mutex.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, nome.as_ptr()) };
        if handle.is_null() {
            // Não deu pra criar o mutex. Melhor deixar o app abrir do que
            // bloquear o usuário por causa da trava.
            return true;
        }

        unsafe { GetLastError() != ERROR_ALREADY_EXISTS }
    }
}

// Fora do Windows (ex.: `cargo check` local) não há mutex nomeado: sempre
// deixa abrir.
#[cfg(not(windows))]
mod imp {
    pub fn adquirir() -> bool {
        true
    }
}

pub use imp::adquirir;
