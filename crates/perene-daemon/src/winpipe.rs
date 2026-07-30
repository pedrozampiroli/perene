//! Transporte IPC do Windows: **named pipes**.
//!
//! No unix o daemon fala por `UnixStream`; no Windows não existe equivalente na
//! std, então implementamos aqui o mínimo necessário sobre a API Win32 — um
//! listener ([`PipeListener`]) e um stream bidirecional ([`PipeStream`]) que
//! implementam `Read`/`Write`, para o resto do daemon (e o cliente na UI) não
//! precisar saber em que plataforma está.
//!
//! **Por que I/O sobreposto (`FILE_FLAG_OVERLAPPED`) e não o modo síncrono
//! simples:** um handle síncrono serializa as operações no mesmo file object, e
//! nós lemos e escrevemos no MESMO stream em threads diferentes (a thread
//! leitora do socket e a que drena o output dos PTYs). Com handle síncrono, uma
//! `ReadFile` bloqueada esperando o próximo comando travaria toda a saída do
//! terminal — o sintoma seria exatamente "o terminal não responde". Com
//! overlapped, cada operação leva o seu próprio `OVERLAPPED` + evento, então
//! leitura e escrita correm de verdade em paralelo.
//!
//! O nome do pipe vem de [`perene_core::paths::daemon_endpoint`]
//! (`\\.\pipe\perene2-<hash do state_dir>`), então testes com `PERENE2_STATE_DIR`
//! próprio ficam isolados igual acontece com o socket no unix (lição #1).

use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_HANDLE_EOF, ERROR_IO_PENDING,
    ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED,
    GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
    OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::CreateEventW;
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

/// Buffer sugerido de cada instância do pipe (só uma dica pro kernel).
const PIPE_BUFFER: u32 = 64 * 1024;
/// Espera máxima por uma instância livre quando o pipe está ocupado.
const BUSY_WAIT_MS: u32 = 5_000;

/// Converte o caminho/nome do pipe para UTF-16 terminado em NUL.
fn wide(name: &Path) -> Vec<u16> {
    name.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn last_error() -> u32 {
    unsafe { GetLastError() }
}

fn io_err(code: u32) -> io::Error {
    io::Error::from_raw_os_error(code as i32)
}

/// Fim de conexão? Nesses códigos a outra ponta fechou — vira EOF, não erro.
fn is_disconnect(code: u32) -> bool {
    matches!(
        code,
        ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED | ERROR_HANDLE_EOF | ERROR_NO_DATA
    )
}

// ── Handles com dono (fecham no Drop) ────────────────────────────────────────

struct OwnedHandle(HANDLE);

// HANDLE é um ponteiro cru, mas é apenas um índice na tabela do processo: pode
// cruzar threads. As operações abaixo são todas thread-safe (cada uma tem seu
// próprio OVERLAPPED).
unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// Evento manual-reset usado como sinal de conclusão de UMA operação.
struct Event(HANDLE);

impl Event {
    fn new() -> io::Result<Self> {
        // (sem SECURITY_ATTRIBUTES, manual-reset, não sinalizado, anônimo)
        let handle = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(handle))
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// Prepara um `OVERLAPPED` zerado apontando para o evento.
fn overlapped(event: &Event) -> OVERLAPPED {
    let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
    ov.hEvent = event.0;
    ov
}

/// Espera a operação sobreposta terminar e devolve quantos bytes passaram.
fn wait_result(handle: HANDLE, ov: &OVERLAPPED) -> io::Result<usize> {
    let mut transferred: u32 = 0;
    let ok = unsafe { GetOverlappedResult(handle, ov, &mut transferred, 1) };
    if ok == 0 {
        let code = last_error();
        if is_disconnect(code) {
            return Ok(0);
        }
        return Err(io_err(code));
    }
    Ok(transferred as usize)
}

// ── Stream ───────────────────────────────────────────────────────────────────

/// Ponta conectada de um named pipe. Clonável: os clones compartilham o mesmo
/// handle (é o análogo do `UnixStream::try_clone`, usado para ter uma thread
/// lendo e outra escrevendo).
#[derive(Clone)]
pub struct PipeStream(Arc<OwnedHandle>);

impl PipeStream {
    fn from_owned(handle: OwnedHandle) -> Self {
        Self(Arc::new(handle))
    }

    fn handle(&self) -> HANDLE {
        self.0 .0
    }

    /// Clone do stream para uso em outra thread (mesma conexão).
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(self.clone())
    }

    /// Aborta o I/O pendente desta conexão — o análogo de
    /// `UnixStream::shutdown`. Uma leitura bloqueada em outra thread retorna
    /// erro na hora, em vez de ficar presa até a outra ponta escrever algo.
    pub fn shutdown(&self) {
        unsafe { CancelIoEx(self.handle(), ptr::null()) };
    }
}

impl Read for PipeStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let event = Event::new()?;
        let mut ov = overlapped(&event);
        let len = buf.len().min(u32::MAX as usize) as u32;
        let ok = unsafe {
            ReadFile(
                self.handle(),
                buf.as_mut_ptr().cast(),
                len,
                ptr::null_mut(),
                &mut ov,
            )
        };
        if ok == 0 {
            let code = last_error();
            if is_disconnect(code) {
                return Ok(0); // outra ponta fechou → EOF
            }
            if code != ERROR_IO_PENDING {
                return Err(io_err(code));
            }
        }
        wait_result(self.handle(), &ov)
    }
}

impl Write for PipeStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let event = Event::new()?;
        let mut ov = overlapped(&event);
        let len = buf.len().min(u32::MAX as usize) as u32;
        let ok = unsafe {
            WriteFile(
                self.handle(),
                buf.as_ptr().cast(),
                len,
                ptr::null_mut(),
                &mut ov,
            )
        };
        if ok == 0 {
            let code = last_error();
            if is_disconnect(code) {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "named pipe desconectado",
                ));
            }
            if code != ERROR_IO_PENDING {
                return Err(io_err(code));
            }
        }
        match wait_result(self.handle(), &ov)? {
            0 => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "named pipe desconectado",
            )),
            n => Ok(n),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        // O kernel já entrega ao ler do outro lado; `FlushFileBuffers` aqui
        // BLOQUEIA até o cliente consumir tudo — o que travaria o daemon se a UI
        // estivesse lenta. Nada a fazer.
        Ok(())
    }
}

// ── Listener ─────────────────────────────────────────────────────────────────

/// Servidor: mantém sempre uma instância do pipe criada e esperando conexão.
///
/// Named pipe no Windows atende UM cliente por instância — a cada `accept` a
/// instância conectada vira o stream do cliente e criamos a próxima.
pub struct PipeListener {
    name: Vec<u16>,
    pending: OwnedHandle,
}

impl PipeListener {
    /// Cria a primeira instância do pipe.
    ///
    /// Usa `FILE_FLAG_FIRST_PIPE_INSTANCE`: se outro daemon já tiver criado esse
    /// pipe, isto falha — segunda barreira de single-instance além do lockfile
    /// (lição #2: dois daemons NUNCA).
    pub fn bind(name: &Path) -> io::Result<Self> {
        let name = wide(name);
        let pending = create_instance(&name, true)?;
        Ok(Self { name, pending })
    }

    /// Bloqueia até um cliente conectar.
    pub fn accept(&mut self) -> io::Result<PipeStream> {
        connect_instance(self.pending.0)?;
        match create_instance(&self.name, false) {
            Ok(next) => {
                let connected = std::mem::replace(&mut self.pending, next);
                Ok(PipeStream::from_owned(connected))
            }
            Err(e) => {
                // Sem a próxima instância não dá pra seguir servindo; devolve a
                // atual ao estado de escuta para o próximo accept tentar de novo.
                unsafe { DisconnectNamedPipe(self.pending.0) };
                Err(e)
            }
        }
    }
}

fn create_instance(name: &[u16], first: bool) -> io::Result<OwnedHandle> {
    let mut open_mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
    if first {
        open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            open_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            PIPE_BUFFER,
            PIPE_BUFFER,
            0,
            // Sem SECURITY_ATTRIBUTES: o pipe herda a DACL padrão do token do
            // usuário (só ele e o SYSTEM alcançam), e REJECT_REMOTE_CLIENTS
            // barra acesso pela rede. Equivale ao socket em `~/.perene2`.
            ptr::null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(OwnedHandle(handle))
}

fn connect_instance(handle: HANDLE) -> io::Result<()> {
    let event = Event::new()?;
    let mut ov = overlapped(&event);
    let ok = unsafe { ConnectNamedPipe(handle, &mut ov) };
    if ok != 0 {
        return Ok(());
    }
    match last_error() {
        ERROR_IO_PENDING => {
            wait_result(handle, &ov)?;
            Ok(())
        }
        // O cliente conectou entre o CreateNamedPipe e o ConnectNamedPipe.
        ERROR_PIPE_CONNECTED => Ok(()),
        code => Err(io_err(code)),
    }
}

// ── Cliente ──────────────────────────────────────────────────────────────────

/// Conecta em um daemon já rodando. `NotFound` = ninguém escutando (o chamador
/// sobe o daemon e tenta de novo).
pub fn connect(name: &Path) -> io::Result<PipeStream> {
    let wide_name = wide(name);
    loop {
        let handle = unsafe {
            CreateFileW(
                wide_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            return Ok(PipeStream::from_owned(OwnedHandle(handle)));
        }
        let code = last_error();
        if code != ERROR_PIPE_BUSY {
            return Err(io_err(code));
        }
        // Todas as instâncias ocupadas: espera uma vagar e tenta de novo.
        if unsafe { WaitNamedPipeW(wide_name.as_ptr(), BUSY_WAIT_MS) } == 0 {
            return Err(io_err(last_error()));
        }
    }
}
